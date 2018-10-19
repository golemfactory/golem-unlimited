angular.module('gu').directive('guProgress', function () {
    return {
        restrict: 'E',
        context: {
            progress: '&'
        },
        require: 'ngModel',
        templateUrl: 'directives/guProgress.html',
        controller: function ($scope) {
            $scope.$watch(() => $scope.progress.leadingTask(true), (newVal, oldVal) => $scope.leading = newVal);

        }
    }
});
