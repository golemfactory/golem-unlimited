
var images = {
    linux: {
        "gu:mining:monero": ["http://10.30.8.179:61622/app/images/monero-linux.tar.gz", "dc4d9e0c100b36c46b6355311ab853996a1448936068283cd7fafb5a90014877"],
        "gu:mining:eth": ["http://10.30.8.179:61622/app/images/eth-linux.tar.gz", "d21f315ed0af9fff9cbc9db38729ebfdcbf16cd7588d39f4f5319e7283cda159"]
    },
    macos: {
        "gu:mining:monero": ["http://10.30.8.179:61622/app/images/monero-macos.tar.gz", "846e1125f927a4817d19e9c1bf34f6ff4ffcf54f6cbeabedbf21e7f59a46d4ac"],
        "gu:mining:eth": ["http://10.30.8.179:61622/app/images/eth-macos.tar.gz", "aabb3fad0ee90eda7cb66473f3579abac35b256276c900d6fbed2050949c6af2"]
    },
}

angular.module('gu')
.run(function(pluginManager) {
    pluginManager.addTab({name: 'Mining', icon: 'plug/Mining/mining.svg', page: 'plug/Mining/base.html'})
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
        if (session.status === 'WORKING') {
            return "plug/Mining/session-working.html"
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
    $scope.progress = miningMan.getProgressAll();
    $scope.mSession = miningMan.session($scope.session.id);

    $scope.mSession.resolveSessions();

    sessionMan.peers($scope.session, true).then(peers => $scope.peers = peers);

   $scope.nextStep = function() {
          $scope.mSession.commit();
          sessionMan.updateSession($scope.session, 'WORKING');
      }

})
.controller('MiningWork', function($scope, $log, sessionMan, miningMan) {
    var session = $scope.$eval('session');
    sessionMan.peers(session, true).then(peers => {
        $scope.peers = peers;
        $scope.mSession.resolveSessions();
    });
    $scope.mSession = miningMan.session(session.id);
    $scope.isCpu = function(rawPeer, type) {
        var peer = $scope.mSession.peer(rawPeer.nodeId);
        //$log.info('isCpu', rawPeer, type);
        return peer.isCpu(type);
    };
    $scope.isGpu = function(rawPeer, type) {
        var peer = $scope.mSession.peer(rawPeer.nodeId);
        return peer.isGpu(type);
    };

    $scope.toggle = function(rawPeer, type, mode) {
        var peer = $scope.mSession.peer(rawPeer.nodeId);

        $log.info('toggle', peer, type, mode);

        if (peer.isWorking(type, mode)) {
            peer.stop(type, mode);
        }
        else {
            peer.start(type, mode);
         }
    };

    console.log('session', session);

})
.service('miningMan', function($log, $interval, $q, sessionMan, hdMan) {

    const TAG_MONERO = 'gu:mining:monero';
    const TAG_ETH = 'gu:mining:eth';

    var progress = {};

    function startProgress(nodeId, tag, estimated, future, label) {
        var nodeProgress = progress[nodeId] || {};
        var tagProgress = nodeProgress[tag] || {};
        var ts = new Date();

        tagProgress.start = ts.getTime();
        tagProgress.end = ts.getTime() + estimated*1000;
        tagProgress.size = tagProgress.end - tagProgress.start;
        tagProgress.label = label;

        nodeProgress[tag] = tagProgress;
        progress[nodeId] = nodeProgress;

        $q.when(future)
        .then(v => $log.info('tag done', tag, nodeId))
        .then(v => delete nodeProgress[tag])
    }

    function getProgressAll() {
        return progress;
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

                    this.initPeers();

                    return peers;
                })
            }
        }

        initPeers() {
            angular.forEach(this.peers, peer => peer.init())
        }

        hr(nodeId, type) {
            //$log.info('hr', nodeId, type);
            var peer =  _.find(this.peers, peer => peer.id === nodeId);
            if (peer) {
                return peer.hr(type);
            }
        }

        peer(nodeId) {
            return _.findWhere(this.peers, {id: nodeId});
        }

        commit() {
            angular.forEach(this.peers, peer => peer.commit());
        }
    }

    class MiningPeer {
        constructor(session, nodeId, details) {
            this.session = session;
            this.id = nodeId;
            this.peer = hdMan.peer(nodeId);
            this.os = details.os;
            this.gpu = details.gpu;
            this.sessions = [];
            // map session.type -> 'pid'
            this.work = this.load('work') || {};
            this.$benchPromise = {};
            this.$afterBench = null;

            if (details.sessions) {
                $this.$init = this.importSessions(details.sessions);
            }
            else {
                $log.warn('no import', details, session);
                this.$init = this.peer.sessions().then(sessions => this.importSessions(sessions));
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

        init() {

            this.$validSession = {};

            $q.when(this.$init).then(v => {
                var p = [];
                angular.forEach(this.sessions, session => {
                var promise = session.validate().then(result => {
                    var type = session.type;
                    $log.info('validate', result);
                    if (!this.$validSession[type]) {
                        this.$validSession[type] = session;
                    }
                    return result;
                });

                promise.then(_v => {
                    var type = session.type;
                    if (this.$validSession[type] === session) {
                        if (!this.hr(session.type)) {
                            $log.info('bench', session.type, this.load('hr'))
                            session.bench();
                        }
                    }
                });

                p.push(promise);
             })

                $q.all(p).then(_v => {
                    angular.forEach(['gu:mining:monero', 'gu:mining:eth'], type => {
                    if (!this.$validSession[type]) {
                        angular.forEach(_.where(this.sessions, {type: type}), session => {
                            if (session !== this.$validSession[type]) {
                                session.drop();
                            }
                        });

                        this.deploy(type).then(session => {
                            session.validate().then(_ => session.bench())
                        })
                    }
                    else {
                        angular.forEach(_.where(this.sessions, {type: type}), session => {
                            if (session !== this.$validSession[type]) {
                                session.drop();
                            }
                        })

                    }
                });
            });
            });
        }

        deploy(type) {
            var promise = sessionMan.getOs(this.id).then(os => {
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

                return rawSession.$create.then(v => {
                    return session;
                })
            });

            startProgress(this.id, 'deploy:' + type, 15, promise, 'installing');

            return promise;
        }

        hr(type) {
            var it = this.load('hr')
            if (it) {
                return it[type];
            }

            return  _.filter(this.sessions, session => session.type === type)
                .map(session => session.hr)
                .find(hr => !!hr);

        }

        save(key, val) {
            window.localStorage.setItem('gu:mining:' + this.id + ':' + key, JSON.stringify(val))
        }

        load(key) {
            var key = 'gu:mining:' + this.id + ':' + key;
            var it = window.localStorage.getItem(key);
            if (it) {
                return JSON.parse(it);
            }

        }

        commit() {
            var hr = {};

            angular.forEach(this.sessions, session => {
                if (session.hr) {
                    hr[session.type] = session.hr;
                }
            });

            this.save('hr', hr);
        }

        bindWork(type, mode, pid) {
            var key = type + ':' + mode;
            if (pid) {
                this.work[key] = pid;
            }
            else {
                delete this.work[key];
            }
            this.save('work', this.work);
        }

        start(type, mode) {
            var session = _.findWhere(this.sessions, {type: type});
            $log.info('start', type, mode, session);
            return session.start(mode).then(r => {
                if (r.Ok) {
                    var pid = r.Ok;
                    this.bindWork(type, mode, pid);
                }
            })
        }

        stop(type, mode) {
            var key = type + ':' + mode;
            var pid = this.work[key];
            var session = _.findWhere(this.sessions, {type: type});
            session.hdSession.stopWithTag('gu:mine:working', pid);
            this.bindWork(type, mode);
        }

        removeSession(session) {
            this.sessions = _.without(this.sessions, session);
        }


        isCpu(type) {
            return this.isWorking(type, 'cpu');
        }

        isGpu(type) {
           return this.isWorking(type, 'gpu');
        }

        isWorking(type, mode) {
            var key = type + ':' + mode;
            return !!this.work[key];
        }

    }

    class MiningPeerSession {

        constructor(peer, id, type) {
            this.peer = peer;
            this.id= id;
            this.type = type;
            this.process = {};
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
            if (!type) {
                if (this.spec.benchmark['DUAL']) {
                    type = 'dual';
                }
                else if (this.spec.benchmark['GPU']) {
                    type = 'gpu';
                }
                else if (this.spec.benchmark['CPU']) {
                    type = 'cpu';
                }
            }
            $log.info('hashRate start');

            if (this.peer.$benchPromise[type]) {
                return this.$benchPromise[type];
            }

            var promise =  $q.when(this.peer.$afterBench).then(_v => {

                var pp = this.hdSession.exec('gu-mine', ['bench-' + type]).then(output => {
                if (output.Ok) {
                    try {
                        var result = JSON.parse(output.Ok);
                        this.hr = result;
                        return {Ok: result};
                    }
                    catch(e) {
                        delete this.$benchPromise;
                        this.status = 'FAIL';
                        $log.error('hr fail', e, output, this);
                        return {Err: 'invalid output'};
                    }
                }
                else {
                    delete this.$benchPromise;
                    this.status = 'FAIL';
                    return output;
                }
            });
                $q.when(this.hdSession.$create).then(_v => startProgress(this.peer.id, 'bench:' + this.type, this.spec.benchmark[type.toUpperCase()], pp, "benchmark"));

                return pp;
            });

            this.peer.$benchPromise[type] = promise;
            this.peer.$afterBench = promise;

            return promise;
        }

        start(type) {
            type = type || 'dual';
            $log.info('mining start');
            return this.hdSession.runWithTag('gu:mine:working', 'gu-mine', ['mine-' + type]).then(output => {
                if (output.Ok) {
                    try {
                        this.status = 'RUNNING';
                        var result = output.Ok[0];
                        this.process = this.process || {};
                        this.process[type] = result;
                        $log.info('start output', result);
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

        drop() {
            this.hdSession.destroy().then(_v =>
                this.peer.removeSession(this)
            )
        }

        status(type) {
            return this.process[type];
        }

        stop(type) {
            var pid = this.process[type];
            return this.hdSession.stopWithTag('gu:mine:working', pid).then(output => {
                if (pid === this.process[type]) {
                    delete this.process[type];
                }
                return output;
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

    interval = $interval(tick, 200);

    function tick() {
        var ts = new Date();

        angular.forEach(progress, (progressNode, nodeId) => {
            angular.forEach(progressNode, (progressInfo, tag) => {
                if (!progressInfo.size) {
                    progressInfo.size = progressInfo.end - progressInfo.start;
                }
                progressInfo.pos = ts.getTime() - progressInfo.start;
                if (progressInfo.pos > progressInfo.size) {
                    progressInfo.size = progressInfo.size * 1.2;
                }
            })
        })
    }

    return { session: session, progress: getProgress, getProgressAll: getProgressAll }
})

