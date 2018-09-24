
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
        {icon: 'glyphicon glyphicon-home', name: 'Status', page: 'status.html'},
        {icon: 'glyphicon glyphicon-th', name: 'Providers', page: 'providers.html'}
      ];
      $scope.pluginTabs = pluginManager.getTabs();

      $scope.activeTab =  $scope.tabs[0];

      $scope.openTab = tab => {
        $scope.activeTab = tab;
      }
  })
  .controller('ProvidersController', function($scope, $http, hubApi) {
     function refresh() {
        $http.get('/peer').then(r => {
            $scope.peers = r.data;
            angular.forEach(r.data, peer => $scope.updatePeer(peer));
        });
     }

     $scope.refresh = refresh;

     $scope.updatePeer = function(peer) {
        hubApi.callRemote(peer.nodeId, 19354, null)
        .then(data=> {
            var ok = data.Ok;
            if (ok) {

                peer.ram = ok.ram;
                peer.gpu = ok.gpu;
                peer.os = ok.os || peer.os;
                peer.hostname = ok.hostname;

            }
        });
        hubApi.callRemote(peer.nodeId, 39, {})
                .then(data=> {
                    var ok = data.Ok;
                    if (ok) {
                        peer.sessions = ok;
                    }
                });
     };

     $scope.peers = [];
     refresh();

  })
  .service('hubApi', function($http) {
        function callRemote(nodeId, destinationId, body) {
            return $http.post('/peer/send-to/' + nodeId + '/' + destinationId, {b: body}).then(r => r.data);
        }

        return { callRemote: callRemote};
  })
  .service('pluginManager', function() {
        var plugins = [];

        function addTab(desc) {
            console.log('add', desc);
            plugins.push(desc)
        }

        function getTabs() {
            console.log('get')
            return plugins;
        }

        return {addTab: addTab, getTabs: getTabs}
  })
  .service('sessionMan', function($http, $log) {
        var sessions = [];
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
            console.log('updateSession', moduleSession, newStatus, updates);
            angular.forEach(sessions, session => {
                if (session.id === moduleSession.id) {
                    if (updates.peers) {
                        session.peers = cleanPeers(updates.peers);
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
            return $http.get('/peer').then(r => r.data);
            if (needDetails) {
                angular.forEach(r.data, peer => peerDetails(peer));
            }
        }

        function peerDetails(peer) {
                hubApi.callRemote(peer.nodeId, 19354, null)
                .then(data=> {
                    if (data.Ok) {
                        peer.ram = data.Ok.ram;
                        peer.gpu = data.Ok.gpu;
                    }
                });
                hubApi.callRemote(peer.nodeId, 39, {})
                        .then(data=> {
                            console.log('d', data)
                        });
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

        return {
            create: create,
            peers: peers,
            sessions: listSessions,
            peerDetails: peerDetails,
            updateSession: updateSession,
            dropSession: dropSession
         }
  })
  .service('hdMan', function($http, hubApi, $q, $log) {
        const HDMAN_CREATE = 37;
        const HDMAN_UPDATE = 38;
        const HDMAN_GET_SESSIONS = 39;

        class HdMan {
            constructor(nodeId) {
                this.nodeId = nodeId;
            }

            newSession(sessionSpec) {
                return new Session(this.nodeId,
                    hubApi.callRemote(this.nodeId, HDMAN_CREATE, sessionSpec));
            }

            fromId(sessionId) {
                return new Session(this.nodeId, {Ok: sessionId});
            }

            sessions() {
                return hubApi.callRemote(this.nodeId, HDMAN_GET_SESSIONS, null)
                    .then(sessions => _.map(sessions, session => this.fromId(session.session_id, session)))
            }
        }

        class Session {
            constructor(nodeId, sessionId) {
                this.nodeId = nodeId;
                this.status = 'PENDING';
                this.$create = $q.when(sessionId).then(id => {
                    if (id.Ok) {
                        this.id = id.Ok;
                        this.status = 'CREATED';
                        return id.Ok;
                    }
                    else {
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
        }

        function peer(nodeId) {
            return new HdMan(nodeId);
        }

        return { peer: peer }
  });
