var app = angular.module('gu', ['ui.bootstrap', 'angularjs-gauge'])
    .filter("prettyJSON", () => json => JSON.stringify(json, null, " "))
    .controller('AppController', function ($scope, pluginManager) {

        function tabActivator(tab) {
            return {
                name: tab.name,
                iconClass: tab.iconClass,
                activator: () => $scope.openTab(tab)
            }
        }

        $scope.tabs = [
            {iconClass: 'glyphicon glyphicon-home', name: 'Status', page: 'status.html'},
            {iconClass: 'gu-providers-icon', name: 'Providers', page: 'providers.html'},
            {iconClass: 'glyphicon glyphicon-list', name: 'Sessions', page: 'sessions.html'},
        ];
        $scope.pluginTabs = pluginManager.getTabs();
        $scope.activeTab = $scope.tabs[0];
        $scope.backList = [];

        function updateBl() {
            var bl = [ tabActivator($scope.tabs[0])];

            if ($scope.activeTab !== $scope.tabs[0]) {
                bl.push(tabActivator($scope.activeTab))
            }

            bl[bl.length-1].act = true;
            $scope.backList = bl;
        };

        $scope.openTab = tab => {
            $scope.activeTab = tab;
            updateBl();
        }
    })

    .controller('GuProvidersController', function ($scope, $interval, $http, $timeout, $uibModal, hubApi, sessionMan) {
        $scope.peers = []
        let timer;
        $scope.refresh = function refresh() {
            sessionMan.peers(null, true).then(peers => {
                timer = $timeout(() => $scope.peers = peers, 100); //hack flicker fix
            })
        };


        $scope.show = function (peer) {
            $uibModal.open({
                animate: true,
                templateUrl: 'hdsession.html',
                controller: function ($scope, $uibModalInstance) {
                    peer.refreshSessions();
                    $scope.peer = peer;
                    $scope.ok = function () {
                        $uibModalInstance.close()
                    };
                    $scope.destroySession = function (peer, session) {
                        session.destroy();
                        peer.refreshSessions();
                    }
                }
            })
        };
        $scope.refresh();
        const intervalPromise =  $interval(() => $scope.refresh(), 5000);

        $scope.$on('$destroy', () => {
            $interval.cancel(intervalPromise);
            timer && $timeout.cancel(timer);
        });

    })
    .controller('StatusController', function ($scope, $http) {
        function refresh() {
            $http.post('/m/19354', "null").then(r => {
                var ok = r.data.Ok;
                if (ok && ok.disk) {
                    if (typeof ok.disk.disk_type !== 'string') ok.disk.disk_type = '';
                    $scope.hub = ok
                }
            });
            $http.get('/peers').then(r => {
                $scope.peers = r.data;
            });
        };

        $scope.refresh = refresh;

        $scope.hub = {};
        refresh();

    })

    .controller('GuSessionsController', function ($scope, $http, $log, $uibModal, pluginManager, hubApi, sessionMan, guProcessMan) {

        let baseTabs = $scope.$eval('backList');

        baseTabs[baseTabs.length-1].activator = function() {
            $scope.currentPage = "sessions-list.html";
            $scope.refresh();
            $scope.backList = baseTabs;
        };

        $scope.backList = baseTabs;

        $scope.refresh = function refresh() {
            hubApi.listSessions().then(sessions => {
                $scope.sessions = sessions;
            })
        };

        $scope.process = function(session) {
            return guProcessMan.getProcess(session);
        };


        $scope.show = function (peer) {
            $uibModal.open({
                animate: true,
                templateUrl: 'hdsession.html',
                controller: function ($scope, $uibModalInstance) {
                    peer.refreshSessions();
                    $scope.peer = peer;
                    $scope.ok = function () {
                        $uibModalInstance.close()
                    };
                    $scope.destroySession = function (peer, session) {
                        session.destroy();
                        peer.refreshSessions();
                    };
                }
            })
        };

        $scope.selectSession = function(session) {

        };

        let context = {
            setPage(session, pageUrl) {
                let name = (typeof session === 'undefined') ? "New Session" : session.id;
                $scope.currentPage = pageUrl;
                $scope.currentSession = session;
                $scope.backList = baseTabs.concat([{name: name}])
            }
        };

        $scope.newSession = function(plugin) {
            plugin.controller('new-session', context);
            $scope.sessionContext = context;
        };

        function haveTags(tags, innerTags) {
            return _.intersection(tags, innerTags).length == innerTags.length;
        }

        $scope.detectPlugin = function(session) {
            return _.filter($scope.plugins, plugin => haveTags(session.tags, plugin.sessionTag));
        };

        $scope.deleteSession = function(session) {
            sessionMan.getSession(session.id).then(session => session.delete()).then(() => $scope.refresh());
        };

        $scope.activateSession = function(session) {
            let plugins = $scope.detectPlugin(session);
            if (plugins.length > 0) {
                sessionMan.getSession(session.id).then(session => {
                    $scope.sessionContext = context;
                    _.forEach(plugins, plugin => plugin.controller('browse', context, session));
                });
            }
        };

        $scope.sessions = [];
        $scope.plugins = pluginManager.getActivators();
        $scope.currentPage = "sessions-list.html";
        $scope.refresh();

    })

    .service('hubApi', function ($http) {
        function callRemote(nodeId, destinationId, body) {
            return $http.post('/peers/send-to/' + nodeId + '/' + destinationId, {b: body}).then(r => r.data);
        }

        function listSessions() {
            return $http.get('/sessions').then(r => r.data);
        }

        return {
            callRemote: callRemote,
            listSessions: listSessions
        };
    })
    .service('pluginManager', function ($log) {
        var tabs = [];
        var activators = [];

        function addTab(desc) {
            $log.info('add', desc);
            tabs.push(desc)
        }

        function getTabs() {
            return tabs;
        }
        
        function addActivator(desc) {
            activators.push(desc);
        }
        
        function getActivators() {
            return $.extend([], activators);
        }

        return {
            addTab: addTab,
            getTabs: getTabs,

            addActivator: addActivator,
            getActivators: getActivators
        };
    })

    .service('hdMan', function ($http, hubApi, $q, $log) {
        var cache = {};

        const HDMAN_CREATE = 37;
        const HDMAN_UPDATE = 38;
        const HDMAN_GET_SESSIONS = 39;
        const HDMAN_DESTROY = 40;

        class HdMan {
            constructor(nodeId) {
                this.nodeId = nodeId;
            }

            newSession(sessionSpec) {
                let spec = angular.copy(sessionSpec);

                if (spec.image && spec.image.cache_file) {
                    delete spec.image.cache_file;
                    spec.image.hash = sessionSpec.image.cache_file;
                }
                spec.envType = 'hd';

                return new Session(this.nodeId,
                    hubApi.callRemote(this.nodeId, HDMAN_CREATE, spec));
            }

            fromId(sessionId, sessionData) {
                return new Session(this.nodeId, {Ok: sessionId}, sessionData);
            }

            sessions() {
                return hubApi.callRemote(this.nodeId, HDMAN_GET_SESSIONS, {})
                    .then(sessions => {
                        var sessions = _.map(sessions.Ok, session => this.fromId(session.id, session));
                        cache[this.nodeId] = sessions;
                        return sessions;
                    })
            }

            sessionsFast() {
                // TODO: FIXME
                if (this.nodeId in cache) {
                    $log.debug("sessionFast: cache used", $q.when(cache[this.nodeId]), this.sessions())
                    return $q.when(cache[this.nodeId]);
                }
                $log.debug("sessionFast: cache not used", this.sessions())
                return this.sessions();
            }
        }

        class Session {
            constructor(nodeId, sessionId, sessionData) {
                this.nodeId = nodeId;
                this.status = 'PENDING';
                this.data = sessionData;
                this.$create = $q.when(sessionId).then(id => {
                    if (id.Ok) {
                        this.id = id.Ok;
                        this.status = 'CREATED';
                        return id.Ok;
                    } else {
                        $log.error('create session fail', id);
                        this.status = 'FAIL';
                        return null;
                    }
                });
            }

            exec(entry, args) {
                return this.$create.then(id =>
                    hubApi.callRemote(this.nodeId, HDMAN_UPDATE, {
                        sessionId: id,
                        commands: [
                            {exec: {executable: entry, args: (args || [])}}
                        ]
                    })
                ).then(result => {
                    $log.info("exec result", result);
                    return result;
                });
            }

            runWithTag(tag, entry, args) {
                return this.$create.then(id =>
                    hubApi.callRemote(this.nodeId, HDMAN_UPDATE, {
                        sessionId: id,
                        commands: [
                            {start: {executable: entry, args: (args || [])}},
                            {addTags: angular.isArray(tag) ? tag : [tag]}
                        ]
                    })
                ).then(result => {
                    $log.info("run_tag result", result);
                    return result;
                });
            }

            stopWithTag(tag, pid) {
                return this.$create.then(id =>
                    hubApi.callRemote(this.nodeId, HDMAN_UPDATE, {
                        sessionId: id,
                        commands: [
                            {stop: {childId: pid}},
                            {delTags: angular.isArray(tag) ? tag : [tag]}
                        ]
                    })
                ).then(result => {
                    $log.info("run_tag result", result);
                    return result;
                });

            }

            destroy() {
                this.status = 'DELETED';

                return hubApi.callRemote(this.nodeId, HDMAN_DESTROY, {session_id: this.id}).then(result => {
                    if (result.Ok) {
                        $log.info("session", this.id, "closed: ", result);
                    } else {
                        $log.error("session", this.id, "closing error:", result);
                    }
                    return result;
                });
            }
        }

        function peer(nodeId) {
            return new HdMan(nodeId);
        }

        return {peer: peer}
    });

fetch('/plug')
    .then(b => b.json())
    .then(plugins => {
        var body = $('body');

        angular.forEach(plugins, plugin => {
            if (plugin.status === 'Active') {
                angular.forEach(plugin.load, loadSrc => {
                    var script = $('<script></script>').attr('src', '/plug/' + plugin.name + '/' + loadSrc);
                    body.append(script);
                });
            }
        });

        $(function () {
            $('<script>angular.bootstrap(document, [\'gu\']);</script>').appendTo(body);
        });
    });
