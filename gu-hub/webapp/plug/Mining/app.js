angular.module('gu')
.run(function(pluginManager) {


    pluginManager.addTab({name: 'Mining', page: 'plug/Mining/base.html'})
})
.controller('MiningController', function($scope) {
    var myStorage = window.localStorage;

    $scope.config = {
        ethAddr: myStorage.getItem('ethAddr', ''),
        moneroAddr: myStorage.getItem('moneroAddr', '')
    };


    $scope.save = function() {
        console.log('s', $scope.config);
        myStorage.setItem('ethAddr',$scope.config.ethAddr);
        myStorage.setItem('moneroAddr',$scope.config.moneroAddr);
    }

    $scope.openSession = function(session) {
        console.log('open', session, $scope.active);
        $scope.session = session;
        $scope.active = 2;

    }

    $scope.onSessions = function() {
        console.log('on sessions');
        delete $scope.session;
    }

})
.controller('MiningSessionsController', function($scope, sessionMan) {
    $scope.sessions = sessionMan.sessions('gu:mining');

    console.log('m', $scope.sessions);

    $scope.newSession = function() {
        sessionMan.create('gu:mining', 'hd');
        $scope.sessions = sessionMan.sessions('gu:mining');
    };

});

