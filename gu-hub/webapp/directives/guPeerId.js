angular.module('gu').directive('guPeerId', function() {

    return {
        restrict: 'A',
        scope: {
            guClick: '@',
        },
        controller: function($scope, $log) {
            $scope.peer= $scope.$parent.$eval('peer');
            $scope.click = function() {
                $scope.$parent.$eval($scope.guClick);
            };

        },
        templateUrl: 'directives/guPeerId.html',
    }

});