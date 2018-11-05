
angular.module('gu')
.directive('guToggle', function() {
    return {
        restrict: 'E',
        require: 'ngModel',
        controller: 'GUToggleController',
        template:
        '<div class="toggle btn btn-xs" ng-class="{\'btn-primary\': val(), \'btn-default\': !val(), \'off\': !val()}">'+
        '<div class="toggle-group" ng-click="toggle()">' +
        '<label class="btn btn-xs btn-primary toggle-on">On</label>' +
        '<label class="btn btn-xs btn-default toggle-off">Off</label>' +
        '<span class="toggle-handle btn btn-xs btn-default"></span>' +
        '</div>',
        link : function($scope, element, attrs, ngModelCtrl){
            $scope.updateModel = function(ngModel) {
                ngModelCtrl.$setViewValue(ngModel);
            }
             $scope.val = function() {
                    return !!ngModelCtrl.$modelValue;
                }
        }

    }
})
.controller('GUToggleController', function($scope) {


    $scope.toggle = function() {
        $scope.updateModel(!$scope.val());
    }

})