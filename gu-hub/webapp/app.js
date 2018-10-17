
var app = angular.module('gu', ['ui.bootstrap', 'angularjs-gauge'])
  .controller('AppController', function($scope, pluginManager) {
      $scope.tabs = [
        {iconClass: 'gu-status-icon', name: 'Status', page: 'status.html'},
        {iconClass: 'gu-providers-icon', name: 'Providers', page: 'providers.html'}
      ];
      $scope.pluginTabs = pluginManager.getTabs();
      $scope.activeTab =  $scope.tabs[0];

      $scope.openTab = tab => {
        $scope.activeTab = tab;
      }
  })
  .controller('ProvidersController', function($scope, $http, $uibModal, hubApi, sessionMan) {

     $scope.refresh = function refresh() {
        sessionMan.peers(null, true).then(peers => $scope.peers = peers)
     }


     $scope.show = function(peer) {
        $uibModal.open({
            animate: true,
            templateUrl: 'hdsession.html',
            controller: function($scope, $uibModalInstance) {
                peer.refreshSessions();
                $scope.peer = peer;
                $scope.ok = function() {
                    $uibModalInstance.close()
                }
                $scope.destroySession = function(peer, session) {
                    session.destroy();
                    peer.refreshSessions();
                }
            }
        })
     }

     $scope.peers = [];
     $scope.refresh();

  })
  .controller('StatusController', function($scope, $http) {
     function refresh() {
        $http.post('/m/19354', "null").then(r => {
            var ok = r.data.Ok;
            if (ok) {
                $scope.hub = ok
            }
        });
        $http.get('/peer').then(r => {
            $scope.peers = r.data;
        });
     };

     $scope.refresh = refresh;

     $scope.hub = {};
     refresh();

  })
  .service('hubApi', function($http) {
        function callRemote(nodeId, destinationId, body) {
            return $http.post('/peer/send-to/' + nodeId + '/' + destinationId, {b: body}).then(r => r.data);
        }

        return { callRemote: callRemote};
  })
  .service('pluginManager', function($log) {
        var plugins = [];

        function addTab(desc) {
            $log.info('add', desc);
            plugins.push(desc)
        }

        function getTabs() {
            $log.info('get')
            return plugins;
        }

        return {addTab: addTab, getTabs: getTabs}
  })
  .service('hdMan', function($http, hubApi, $q, $log) {
        var cache = {};

        const HDMAN_CREATE = 37;
        const HDMAN_UPDATE = 38;
        const HDMAN_GET_SESSIONS = 39;
        const HDMAN_DESTROY = 40;

        class HdMan {
            constructor(nodeId) {
                this.nodeId = nodeId;
            }

            newSession(sessionSpec) {
                return new Session(this.nodeId,
                    hubApi.callRemote(this.nodeId, HDMAN_CREATE, sessionSpec));
            }

            fromId(sessionId, sessionData) {
                return new Session(this.nodeId, {Ok: sessionId}, sessionData);
            }

            sessions() {
                return hubApi.callRemote(this.nodeId, HDMAN_GET_SESSIONS, {})
                    .then(sessions => {
                        var sessions = _.map(sessions.Ok, session => this.fromId(session.id, session));
                        cache[this.nodeId] = sessions;
                        return sessions;
                    })
            }

            sessionsFast() {
            // TODO: FIXME
                if (this.nodeId in cache) {
                    $log.debug("sessionFast: cache used", $q.when(cache[this.nodeId]), this.sessions())
                    return $q.when(cache[this.nodeId]);
                }
                $log.debug("sessionFast: cache not used", this.sessions())
                return this.sessions();
            }
        }

        class Session {
            constructor(nodeId, sessionId, sessionData) {
                this.nodeId = nodeId;
                this.status = 'PENDING';
                this.data = sessionData;
                this.$create = $q.when(sessionId).then(id => {
                    if (id.Ok) {
                        this.id = id.Ok;
                        this.status = 'CREATED';
                        return id.Ok;
                    }
                    else {
                        $log.error('create session fail', id);
                        this.status = 'FAIL';
                        return null;
                    }
                });
            }

            exec(entry, args) {
                return this.$create.then(id =>
                    hubApi.callRemote(this.nodeId, HDMAN_UPDATE, {
                        session_id: id,
                        commands: [
                            {Exec: {executable: entry, args: (args||[])}}
                        ]
                    })
                ).then(result => {
                    $log.info("exec result", result);
                    return result;
                });
            }

            runWithTag(tag, entry, args) {
                return this.$create.then(id =>
                    hubApi.callRemote(this.nodeId, HDMAN_UPDATE, {
                        session_id: id,
                        commands: [
                            {Start: {executable: entry, args: (args||[])}},
                            {AddTags: angular.isArray(tag) ? tag : [tag]}
                        ]
                    })
                ).then(result => {
                    $log.info("run_tag result", result);
                    return result;
                });
            }

            stopWithTag(tag, pid) {
                return this.$create.then(id =>
                    hubApi.callRemote(this.nodeId, HDMAN_UPDATE, {
                        session_id: id,
                        commands: [
                            {Stop: {child_id: pid}},
                            {DelTags: angular.isArray(tag) ? tag : [tag]}
                        ]
                    })
                ).then(result => {
                    $log.info("run_tag result", result);
                    return result;
                });

            }

            destroy() {
                this.status = 'DELETED';

                return hubApi.callRemote(this.nodeId, HDMAN_DESTROY, {session_id : this.id}).then(result => {
                    if (result.Ok) {
                        $log.info("session", this.id, "closed: ", result);
                    } else {
                        $log.error("session", this.id, "closing error:", result);
                    }
                    return result;
                });
            }
        }

        function peer(nodeId) {
            return new HdMan(nodeId);
        }

        return { peer: peer }
  });

fetch('/plug')
.then(b => b.json())
.then(plugins => {
    var body = $('body');

    angular.forEach(plugins, plugin => {
        if (plugin.status === 'Active') {
            angular.forEach(plugin.load, loadSrc => {
                var script = $('<script></script>').attr('src', '/plug/' + plugin.name + '/' + loadSrc);
                body.append(script);
            });
        }
    })
    $('<script>angular.bootstrap(document, [\'gu\']);</script>').appendTo(body);
})
