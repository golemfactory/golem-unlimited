
function guid() {
  function s4() {
    return Math.floor((1 + Math.random()) * 0x10000)
      .toString(16)
      .substring(1);
  }
  return s4() + s4() + '-' + s4() + '-' + s4() + '-' + s4() + '-' + s4() + s4() + s4();
}

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
  .service('sessionMan', function($http, $log, $q, hubApi, hdMan) {
        var sessions = [];
        var osMap = {};
        if ('gu:sessions' in window.localStorage) {
            sessions = JSON.parse(window.localStorage.getItem('gu:sessions'));
        }

        function save(newSessions) {
            if (angular.isArray(newSessions)) {
                sessions = newSessions;
            }
            $log.info('save', sessions);
            window.localStorage.setItem('gu:sessions', JSON.stringify(sessions));
        }

        function cleanPeer(peer) {
            return {nodeId: peer.nodeId};
        }

        function cleanPeers(inPeers) {
            var peers = [];
            angular.forEach(inPeers, peer => peers.push(cleanPeer(peer)));

            return peers;
        }

        function updateSession(moduleSession, newStatus, updates) {
            $log.info('updateSession', moduleSession, newStatus, updates);
            angular.forEach(sessions, session => {
                if (session.id === moduleSession.id) {
                    if (updates) {
                        if (updates.peers) {
                            session.peers = cleanPeers(updates.peers);
                        }
                    }
                    session.status = newStatus;
                    angular.copy(session, moduleSession);
                }
            });
            save();
        }

        function dropSession(moduleSession) {
            $log.info("drop", moduleSession);
            save(_.reject(sessions, session => session.id === moduleSession.id));
        }

        function create(sessionType, env) {
            var session = {
                id: guid(),
                type: sessionType,
                env: env,
                status: 'NEW'
            };
            sessions.push(session);
            save();
        }

        function send(node_id) {
            return function(destination, body) {
                return $http.post('/peer/send-to', [node_id, destination, body]).then(r => r.data);
            }
        }

        function peers(session, needDetails) {
            var peersPromise;

            if (session && session.status !== 'NEW' && session.peers) {
                peersPromise = $q.when(_.map(session.peers, peer => angular.copy(peer)));
            }
            else {
                peersPromise = $http.get('/peer').then(r => r.data);
            }

            if (needDetails) {
                peersPromise.then(peers => angular.forEach(peers, peer => peerDetails(peer)));
            }

            return peersPromise;
        }

        function peerDetails(peer) {
            hubApi.callRemote(peer.nodeId, 19354, null).then(data=> {
                var ok = data.Ok;
                if (ok) {
                    peer.ram = ok.ram;
                    peer.gpu = ok.gpu;
                    peer.os = ok.os || peer.os;
                    osMap[peer.nodeId] = ok.os || peer.os || 'unk';
                    peer.hostname = ok.hostname;
                }
            });

            peer.hdMan = hdMan.peer(peer.nodeId);
            peer.refreshSessions = function() {
                peer.hdMan.sessions().then(sessions => peer.sessions = sessions);
            }

            peer.refreshSessions();
        };


        function listSessions(sessionType) {
            var s = [];

            angular.forEach(sessions, session => {
                if (sessionType === session.type) {
                    var sessionDto = angular.copy(session);
                    sessionDto.send = send(session.nodeId);
                    s.push(sessionDto);
                }
            });
            return s;
        }

        function getSession(sessionId) {
            return  _.find(sessions, session => session.id === sessionId);
        }

        function getOs(nodeId) {
            if (nodeId in osMap) {
                return $q.when(osMap[nodeId]);
            }
            else {
                return hubApi.callRemote(nodeId, 19354, null).then(data=> {
                    var ok = data.ok;
                    if (ok) {
                        osMap[peer.nodeId] = ok.os || peer.os || 'unk';
                    }
                    return osMap[peer.nodeId];
                });
            }
        }

        return {
            create: create,
            peers: peers,
            sessions: listSessions,
            peerDetails: peerDetails,
            updateSession: updateSession,
            dropSession: dropSession,
            getSession: getSession,
            getOs: getOs
         }
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
