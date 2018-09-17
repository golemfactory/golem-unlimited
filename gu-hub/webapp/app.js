
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
  .controller('ProvidersController', function($scope, $http) {
     $http.get('/peer').then(r => {
        $scope.peers = r.data;

     })

     $scope.peers = [];

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

        function peers(session) {
            return $http.get('/peer').then(r => r.data);
        }

        function listSessions(sessionType) {
            var s = [];

            angular.forEach(sessions, session => {
                if (sessionType === session.type) {
                    s.push(angular.copy(session));
                }
            });
            return s;
        }

        return { create: create, peers: peers, sessions: listSessions }
  });
