angular.module('gu').directive('guListProvider', function () {
    return {
        restrict: 'E',
        scope: {
            peers: '=ngModel'
        },
        require: 'ngModel',
        templateUrl: 'directives/guListProvider.html',
        controller: function ($scope, sessionMan, $q) {
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
}).directive('animateOnChange', function($timeout) {
    return function(scope, element, attr) {
        scope.$watch(attr.animateOnChange, function(newValue, oldValue) {
            if (newValue != oldValue) {
                element.removeClass(attr.animClass);
                $timeout(function() {
                    element.addClass(attr.animClass);
                }, attr.timeout);
            }
        });
    };
});
