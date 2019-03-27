angular.module('gu').directive('guListProvider', function () {
    return {
        restrict: 'E',
        scope: {
            peers: '=ngModel'
        },
        require: 'ngModel',
        templateUrl: 'directives/guListProvider.html',
        controller: function ($scope, sessionMan, $q) {
            console.log('ddd');
            $scope.hubPeers = [];
            $scope.statusMap = {};


            function initPeers(peers) {
                console.log('p=', peers);
                sessionMan.selectedPeers(peers).then(peers => $scope.hubPeers = peers);
            }



            $scope.$watch('peers', (v, ov) => {
                initPeers(v);
            });


            initPeers($scope.peers);
        }
    }
});