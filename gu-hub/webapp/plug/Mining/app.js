
var images = {
    linux: "http://10.30.8.179:61622/app/images/monero-linux.tar.gz",
    macos: "http://10.30.8.179:61622/app/images/monero-macos.tar.gz",
}

angular.module('gu')
.run(function(pluginManager) {
    pluginManager.addTab({name: '💎 Mining', page: 'plug/Mining/base.html'})
})
.controller('MiningController', function($scope) {
    var myStorage = window.localStorage;

    $scope.config = {
        ethAddr: myStorage.getItem('ethAddr', ''),
        moneroAddr: myStorage.getItem('moneroAddr', '')
    };

    $scope.sessionPage=function(session) {
        var session = $scope.session;

        if (!session) {
            return;
        }

        if (session.status === 'NEW') {
            return "plug/Mining/new-session.html";
        }
        if (session.status === 'CREATED') {
            return "plug/Mining/prepare-session.html"
        }
    }

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
.controller('MiningSessionsController', function($scope, sessionMan, $uibModal, $log) {
    $scope.sessions = sessionMan.sessions('gu:mining');
    $scope.newSession = function() {
        sessionMan.create('gu:mining', 'hd');
        $scope.sessions = sessionMan.sessions('gu:mining');
    };

    function reload() {
        $scope.sessions = sessionMan.sessions('gu:mining');
    }

    $scope.dropSession = function(session) {
        $uibModal.open({
            animate: true,
            templateUrl: 'modal-confirm.html',
            controller: function($scope, $uibModalInstance) {
                $scope.title = 'Session delete';
                $scope.question = 'Delete session ' + session.id + ' ?';
                $scope.ok = function() {
                    sessionMan.dropSession(session);
                    reload();
                    $uibModalInstance.close()
                }
            }
        })
    }

})
.controller('MiningPreselection', function($scope, sessionMan) {
    $scope.peers = [];
    $scope.all = false;
    $scope.session = $scope.$parent.$parent.$parent.$parent.session;

    $scope.$watch('all', function(v) {
        if ($scope.all) {
            angular.forEach($scope.peers, peer => peer.assigned=true)
        } else {
            angular.forEach($scope.peers, peer => peer.assigned=false)
        }
    });

    $scope.blockNext = function(peers) {
        console.log('a', $scope.peers.some(peer => !!peer.assigned));
        return !peers.some(peer => !!peer.assigned);
    }

    $scope.nextStep = function() {
        sessionMan.updateSession($scope.session, 'CREATED', {peers: _.filter($scope.peers, peer => peer.assigned)})
    }

    sessionMan.peers($scope.session, true).then(peers => $scope.peers = peers);
})
.controller('MiningPrepare', function($scope, $log, $interval, sessionMan, hdMan) {

    $scope.session = $scope.$parent.$parent.$parent.$parent.session;
    $scope.peers = $scope.session.peers;
    $scope.progress = {};
    $scope.monero = {};

    $scope.runBenchmark = function(peer) {
        var nodeId = peer.nodeId;
        var progress = {step: 0, max: 20, label: 'installing'};
        $scope.progress[nodeId] = progress;
        var peer = hdMan.peer(peer.nodeId);
        var benchmark = null;

        var peerSession = peer.newSession({
            name: 'gu:mining monero',
            image: {cache_file: 'm.tar.gz', url: images.linux},
            tags: ['gu:mining', 'gu:mining:monero']
         });

        peerSession.exec('gu-mine', ['bench-cpu']).then(r => {
            benchmark = parseInt(r.Ok[0].match(/Benchmark Total: ([0-9]+.[0-9]*) H/)[1]);
            window.r = r;
        })

        var interval;
        var unit = 200.0/1000.0;

        function tick() {
            if (progress.label === 'installing') {
                if (peerSession.status === 'PENDING') {
                    progress.step += unit;
                    if (progress.step>=progress.max) {
                        progress.max = progress.max*1.5;
                    }
                }
                else {
                    progress.step = 0;
                    progress.max = 92;
                    progress.label ="benchmark";
                }
            }
            if (progress.label === 'benchmark') {
                if (benchmark === null) {
                    progress.step += unit;
                    if (progress.step>=progress.max) {
                        progress.max = progress.max*1.5;
                    }
                }
                else {
                    progress.step = progress.max;
                    progress.label ="done";
                    delete $scope.progress[nodeId];
                    $scope.monero[nodeId] = {hashRate: benchmark};
                }
            }
        }

        interval = $interval(tick, 200, 120*5, true);
    };
    console.log('s', $scope);
});

