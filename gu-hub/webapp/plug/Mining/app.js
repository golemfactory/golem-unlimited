
var images = {
    linux: {"gu:mining:monero": ["http://10.30.8.179:61622/app/images/monero-linux.tar.gz", "dc4d9e0c100b36c46b6355311ab853996a1448936068283cd7fafb5a90014877"]},
    macos: {"gu:mining:monero": ["http://10.30.8.179:61622/app/images/monero-macos.tar.gz", "846e1125f927a4817d19e9c1bf34f6ff4ffcf54f6cbeabedbf21e7f59a46d4ac"]},
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
.controller('MiningPrepare', function($scope, $log, $interval, sessionMan, hdMan, miningMan) {

    $scope.session = $scope.$parent.$parent.$parent.$parent.session;

    $scope.peers = [];
    $scope.progress = {};
    $scope.monero = {};
    $scope.mSession = miningMan.session($scope.session.id);

    $scope.mSession.resolveSessions();

    sessionMan.peers($scope.session, true).then(peers => $scope.peers = peers);

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
})
.service('miningMan', function($log, sessionMan, hdMan) {

    const TAG_MONERO = 'gu:mining:monero';
    const TAG_ETH = 'gu:mining:eth';

    var progress = {};

    function startProgress(nodeId, tag, estimated, future, label) {
        var nodeProgress = progress[nodeId] || {};
        var tagProgress = nodeProgress[tag] || {};
        var ts = new Date();

        tagProgress.start = ts.getTime();
        tagProgress.end = ts.getTime() + estimated*1000;
        tagProgress.label = label;

        nodeProgress[tag] = tagProgress;
        progress[nodeId] = nodeProgress;

        $q.when(future).then(v => delete nodeProgress[tag])
    }

    function getProgress(nodeId, tag) {
        var nodeProgress = progress[nodeId] || {};
        var tagProgress = nodeProgress[tag] || {};

        return tagProgress;
    }

    function isMiningSession(session) {
        return _.any(session.data.tags, tag => tag === 'gu:mining');
    }

    function getSessionType(session) {
            if (_.any(session.data.tags, tag => tag === TAG_MONERO)) {
                return TAG_MONERO;
            }
            if (_.any(session.data.tags, tag => tag === TAG_ETH)) {
                return TAG_ETH;
            }
    }

    class MiningSession {
        constructor(id) {
            this.id = id;
            this.session = sessionMan.getSession(id);
            this.peers = [];
            this.$resolved=false;
        }

        resolveSessions() {
            if (!this.$resolved) {
                sessionMan.peers(this.session, true).then(peers => {
                    $log.info('resolved peers', this.session, peers);
                    this.peers = _.map(peers, peer => new MiningPeer(this, peer.nodeId, peer));
                    this.$resolved = true;
                    return peers;
                })
            }
        }

        hr(nodeId, type) {
            $log.info('hr', nodeId, type);
            var peer =  _.find(this.peers, peer => peer.id === nodeId);
            if (peer) {
                $log.info('hr peer', peer, nodeId, type);
                return peer.hr(type);
            }
        }
    }

    class MiningPeer {
        constructor(session, nodeId, details) {
            this.session = session;
            this.id = nodeId;
            this.peer = hdMan.peer(nodeId);
            this.os = details.os;
            this.gpu = details.gpu;
            if (details.sessions) {
                this.importSessions(details.sessions);
            }
            else {
                $log.warn('no import', details, session);
                this.peer.sessions().then(sessions => this.importSessions(sessions));
            }
        }

        importSessions(rawHdManSessions) {
            $log.info('rawSessions', rawHdManSessions);
            this.sessions = [];
            angular.forEach(rawHdManSessions, rawSession => {
                if (isMiningSession(rawSession)) {
                    var session = new MiningPeerSession(this, rawSession.id, getSessionType(rawSession));
                    session.hdSession = rawSession;
                    this.sessions.push(session);
                }
            });
        }

        deploy(type) {
            sessionMan.getOs(this.id).then(os => {
                var image = images[os.toLowerCase()][type];
                $log.info('image', image, os.toLowerCase(), images[os.toLowerCase()], type);
                $log.info('hd peer', typeof this.peer, this.peer);
                var rawSession = this.peer.newSession({
                    name: 'gu:mining ' + type,
                    image: {cache_file: image[1] + '.tar.gz', url: image[0]},
                    tags: ['gu:mining', type]
                });
                var session = new MiningPeerSession(this, rawSession.id, type);
                session.hdSession = rawSession;
                this.sessions.push(session);
            });
        }

        hr(type) {
            $log.info('hr peer', type, this.sessions);
            return  _.filter(this.sessions, session => session.type === type)
                .map(session => session.hr)
                .find(hr => !!hr);

        }
    }

    class MiningPeerSession {

        constructor(peer, id, type) {
            this.peer = peer;
            this.id= id;
            this.type = type;
        }

        validate() {
            $log.info('validate start');
            return this.hdSession.exec('gu-mine', ['spec'])
                .then(output => {
                    if (output.Ok) {
                        try {
                            var spec = JSON.parse(output.Ok);
                            this.spec = spec;
                            return {Ok: spec};
                        }
                        catch(e) {
                            this.status = 'FAIL';
                            return {'Err': 'invalid output'};
                        }
                    }
                    else {
                        this.status = 'FAIL';
                        return output;
                    }
                });
        }

        bench(type) {
            type = type || 'dual';
            $log.info('hashRate start');
            return this.hdSession.exec('gu-mine', ['bench-' + type]).then(output => {
                if (output.Ok) {
                    try {
                        var result = JSON.parse(output.Ok);
                        this.hr = result;
                        return {Ok: result};
                    }
                    catch(e) {
                        this.status = 'FAIL';
                        $log.error('hr fail', e, output, this);
                        return {Err: 'invalid output'};
                    }
                }
                else {
                    this.status = 'FAIL';
                    return output;
                }
            })
        }

        start(type) {
            type = type || 'dual';
            $log.info('mining start');
            return this.hdSession.runWithTag('gu:mine:working', 'gu-mine', ['mine-' + type]).then(output => {
                if (output.Ok) {
                    try {
                        this.status = 'RUNNING';
                        var result = JSON.parse(output.Ok);
                        this.process = this.process || {};
                        this.process[type] = result;
                        return {Ok: result};
                    }
                    catch(e) {
                        this.status = 'FAIL';
                        $log.error('hr fail', e, output, this);
                        return {Err: 'invalid output'};
                    }
                }
                else {
                    this.status = 'FAIL';
                    return output;
                }
            })
        }
    }

    var cache = {};

    function session(id) {
        if (id in cache) {
            return cache[id];
        }
        cache[id] = new MiningSession(id);
        return cache[id];
    }

    return { session: session, progress: getProgress }
})

