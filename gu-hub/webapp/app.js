
function guid() {
  function s4() {
    return Math.floor((1 + Math.random()) * 0x10000)
      .toString(16)
      .substring(1);
  }
  return s4() + s4() + '-' + s4() + '-' + s4() + '-' + s4() + '-' + s4() + s4() + s4();
}

var app = angular.module('gu', ['ui.bootstrap'])
  .controller('AppController', function($scope, pluginManager) {
      $scope.tabs = [{name: 'Status', page: 'status.html'}, {name: 'Providers', page: 'providers.html'}];
      $scope.pluginTabs = pluginManager.getTabs();

      $scope.activeTab =  $scope.tabs[0];

      $scope.openTab = tab => {
        $scope.activeTab = tab;
      }
  })
  .controller('ProvidersController', function($scope, $http, hubApi) {
     $http.get('/peer').then(r => {
        $scope.peers = r.data;
        angular.forEach(r.data, peer => $scope.updatePeer(peer));

     });

     $scope.updatePeer = function(peer) {
        hubApi.callRemote(peer.nodeId, 19354, null)
        .then(data=> {
            if (data.Ok) {
                peer.ram = data.Ok.ram;
                peer.gpu = data.Ok.gpu;
            }
        });
        hubApi.callRemote(peer.nodeId, 39, {})
        //$http.post('/peer/send-to', {nodeId: peer.nodeId, destinationId: 39, body: {}})
                .then(data=> {
                    console.log('d', data)
                });


     };

     $scope.peers = [];

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
  .service('sessionMan', function($http) {
        var sessions = [];
        if ('gu:sessions' in window.localStorage) {
            sessions = JSON.parse(window.localStorage.getItem('gu:sessions'));
        }

        function save() {
            window.localStorage.setItem('gu:sessions', JSON.stringify(sessions));
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

        function peers(session) {
            return $http.get('/peer').then(r => r.data);
        }

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

        return { create: create, peers: peers, sessions: listSessions }
  });
