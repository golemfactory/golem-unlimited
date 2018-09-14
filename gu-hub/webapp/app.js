var app = angular.module('gu', ['ui.bootstrap.tabs'])
  .controller('AppController', function($scope) {
      $scope.tabs = [{name: 'Status', page: 'status.html'}, {name: 'Providers', page: 'providers.html'}];
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

  });
  angular.bootstrap(document, ['gu']);