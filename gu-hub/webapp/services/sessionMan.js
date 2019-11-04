angular.module('gu')
    .service('sessionMan', function ($http, $log, $q, hubApi, guPeerMan) {

        function guid() {
            function s4() {
                return Math.floor((1 + Math.random()) * 0x10000)
                    .toString(16)
                    .substring(1);
            }

            return s4() + s4() + '-' + s4() + '-' + s4() + '-' + s4() + '-' + s4() + s4() + s4();
        }

        var sessions = [];
        var osMap = {};
        if ('gu:sessions' in window.localStorage) {
            sessions = JSON.parse(window.localStorage.getItem('gu:sessions'));
        }

        function save(newSessions) {
            if (angular.isArray(newSessions)) {
                sessions = newSessions;
            }
            $log.info('save', sessions);
            window.localStorage.setItem('gu:sessions', JSON.stringify(sessions));
        }

        function cleanPeer(peer) {
            return {nodeId: peer.nodeId};
        }

        function cleanPeers(inPeers) {
            var peers = [];
            angular.forEach(inPeers, peer => peers.push(cleanPeer(peer))
            )
            ;

            return peers;
        }

        function updateSession(moduleSession, newStatus, updates) {
            $log.info('updateSession', moduleSession, newStatus, updates);
            angular.forEach(sessions, session => {
                if (session.id === moduleSession.id) {
                    if (updates) {
                        if (updates.peers) {
                            session.peers = cleanPeers(updates.peers);
                        }
                        if (updates.config) {
                            session.config = angular.copy(updates.config)
                        }
                    }
                    session.status = newStatus;
                    angular.copy(session, moduleSession);
                }
            })
            ;
            save();
        }

        function dropSession(moduleSession) {
            $log.info("drop", moduleSession);
            save(_.reject(sessions, session => session.id === moduleSession.id)
            )
            ;
        }

        function create(name, tags) {
            const spec = {name: name, tags: tags || [], allocation: 'manual'};
            return $http.post('/sessions', spec)
                .then((response) => new HubSession(response.data, spec));
        }

        function send(node_id) {
            return function (destination, body) {
                return $http.post('/peers/send-to', [node_id, destination, body]).then(r => r.data
                )
                    ;
            }
        }

        function allPeers() {
            const peersPromise = $http.get('/peers').then(r => r.data);

            peersPromise.then(peers => angular.forEach(peers, peer => hubApi.callRemote(peer.nodeId, 19354, null).then(data => {
                var ok = data.Ok;
                if (ok) {
                    peer.ram = ok.ram;
                    peer.gpu = ok.gpu;
                    peer.os = ok.os || peer.os;
                    osMap[peer.nodeId] = ok.os || peer.os || 'unk';
                    peer.hostname = ok.hostname;
                    peer.num_cores = ok.num_cores;
                }
            })));

            return peersPromise;
        }

        function selectedPeers(peers) {

            console.log('selected peers for', peers);

            function isInList(nodeId) {
                let v = _.any(peers, p => p.nodeId ? p.nodeId === nodeId : p === nodeId);
                console.log('nodeId=', nodeId, 'peers=', peers, 'v=', v);
                return v;
            }

            const peersPromise = $http.get('/peers').then(r => r.data)
                .then(peers => _.filter(peers, peer => isInList(peer.nodeId)))
                .then(peers => {
                    return peers;
                });

            peersPromise.then(peers => angular.forEach(peers, peer => hubApi.callRemote(peer.nodeId, 19354, null).then(data => {
                var ok = data.Ok;
                if (ok) {
                    peer.ram = ok.ram;
                    peer.gpu = ok.gpu;
                    peer.os = ok.os || peer.os;
                    osMap[peer.nodeId] = ok.os || peer.os || 'unk';
                    peer.hostname = ok.hostname;
                    peer.num_cores = ok.num_cores;
                }
            })));

            return peersPromise;

        }

        function peers(session, needDetails) {
            var peersPromise;

            if (session && session.status !== 'NEW' && session.peers) {
                peersPromise = $q.when(_.map(session.peers, peer => angular.copy(peer))
                )
                ;
            } else {
                peersPromise = $http.get('/peers').then(r => r.data)
                ;
            }

            if (needDetails) {
                peersPromise.then(peers => angular.forEach(peers, peer => peerDetails(session, peer)));
            }

            return peersPromise;
        }

        function peerDetails(session, peer) {
            hubApi.callRemote(peer.nodeId, 19354, null).then(data => {
                var ok = data.Ok;
                if (ok) {
                    peer.ram = ok.ram;
                    peer.gpu = ok.gpu;
                    peer.os = ok.os || peer.os;
                    osMap[peer.nodeId] = ok.os || peer.os || 'unk';
                    peer.hostname = ok.hostname;
                    peer.num_cores = ok.num_cores;
                }
            })
            ;

            peer.manager = guPeerMan.peer(session, peer.nodeId);
            peer.refreshSessions = function () {
                peer.manager.sessions().then(sessions => peer.sessions = sessions);
            };

            peer.refreshSessions();
        }


        function listSessions(sessionType) {
            var s = [];

            angular.forEach(sessions, session => {
                if (sessionType === session.type
                ) {
                    var sessionDto = angular.copy(session);
                    sessionDto.send = send(session.nodeId);
                    s.push(sessionDto);
                }
            })
            ;
            return s;
        }

        function getSession(sessionId) {
            return $http.get(`/sessions/${sessionId}`).then(response => {
                return new HubSession(sessionId, response.data);
            })
        }

        function getOs(nodeId) {
            if (nodeId in osMap) {
                return $q.when(osMap[nodeId]);
            } else {
                return hubApi.callRemote(nodeId, 19354, null).then(data => {
                    var ok = data.ok;
                    if (ok) {
                        osMap[peer.nodeId] = ok.os || peer.os || 'unk';
                    }
                    return osMap[peer.nodeId];
                });
            }
        }


        class HubSession {
            constructor(sessionId, spec) {
                this.id = sessionId;
                this.spec = spec;
            }

            setConfig(config) {
                return $http.put(`/sessions/${this.id}/config`, config).then(response => {
                    return response.data;
                })
            }

            getConfig() {
                return $http.get(`/sessions/${this.id}/config`).then(response => response.data);
            }

            updateConfig(updateFn) {
                return this.getConfig().then(data => {
                    updateFn(data);
                    this.setConfig(data)
                })
            }

            peers() {
                return $http.get(`/sessions/${this.id}/peers`).then(response => _.map(response.data, peer => new HubSessionPeer(this, peer.nodeId, peer)));
            }

            addPeers(peers) {

                let sessionId = this.id;

                function addPeersInner(peers) {
                    return $http.post(`/sessions/${sessionId}/peers`, peers).then(response => response.data);
                }

                if (Array.isArray(peers)) {
                    return addPeersInner(peers);
                }
                if (typeof peers === 'string') {
                    return addPeersInner(arguments);
                }
                throw 'invalid peer list';
            }

            delete() {
                return $http.delete(`/sessions/${this.id}`).then(response => null);
            }

            tagValue(name, value) {
                let prefix = `${name}=`;
                let tag = _.find(this.spec.tags, tag => tag.startsWith(prefix));
                if (typeof value === 'undefined') {
                    if (tag) {
                        return tag.substring(prefix.length);
                    }
                    return undefined;
                }
            }

            get _url() {
                return `/sessions/${this.id}`;
            }
        }

        class HubSessionPeer {
            constructor(hubSession, nodeId, peerInfo) {
                this.hubSession = hubSession;
                this.nodeId = nodeId;
                this.peerInfo = peerInfo;
            }

            get id() {
                return this.nodeId;
            }

            get os() {
                if (this._hardware === undefined) {
                    this._refresh();
                }
                return this._hardware.Ok && this._hardware.Ok.os;
            }

            get ram() {
                if (this._hardware === undefined) {
                    this._refresh();
                }
                return this._hardware.Ok && this._hardware.Ok.ram;
            }

            get hostName() {

                return this._hardware.Ok && this._hardware.Ok.hostname;
            }

            get numCores() {
                return this._hardware.Ok && this._hardware.Ok.num_cores;
            }

            get gpu() {
                if (this._hardware === undefined) {
                    this._refresh();
                }

                return this._hardware.Ok && this._hardware.Ok.gpu;
            }

            get deployments() {
                return $http.get(`${this._url}/deployments`).then(response => {
                    let deployments = response.data;

                    return deployments.map(deployment => new HubSessionDeployment(this, deployment.id, deployment));
                });
            }

            async createDeployment(spec) {
                let response = await $http.post(`${this._url}/deployments`, spec);

                return new HubSessionDeployment(this, response.data.deploymentId, response.data);
            }

            async createHdDeployment(spec) {
                let nativeSpec = {
                    name: spec.name,
                    tags: spec.tags
                };

                nativeSpec.envType = 'hd';
                let os = await getOs(this.nodeId);
                nativeSpec.image = spec.images[os];
                return await this.createDeployment(nativeSpec);
            }

            get _url() {
                return `${this.hubSession._url}/peers/${this.nodeId}`;
            }

            async _refresh() {
                if (this._hardware === undefined) {
                    this._hardware = {};
                    this._hardware = await hubApi.callRemote(this.nodeId, 19354, null);
                }
            }
        }

        class HubSessionDeployment {

            constructor(hubPeer, deploymentId, deploymentInfo) {
                this.hubPeer = hubPeer;
                this.deploymentId = deploymentId;
                if (typeof deploymentInfo === "object") {
                    this.deploymentInfo = deploymentInfo;
                }
            }

            async update(commands) {
                return (await $http.patch(this._url, commands)).data
            }

            async tags(refresh) {
                if (refresh || !!this.deploymentInfo) {
                    let response = await $http.get(this._url);

                    this.deploymentInfo = response.data;
                }

                return [...this.deploymentInfo.tags];
            }

            get node() {
                return this.hubPeer;
            }

            delete() {
                return $http.delete(this._url);
            }

            get _url() {
                return `${this.hubPeer._url}/deployments/${this.deploymentId}`;
            }
        }


        return {
            create: create,
            peers: peers,
            allPeers: allPeers,
            selectedPeers: selectedPeers,
            sessions: listSessions,
            peerDetails: peerDetails,
            updateSession: updateSession,
            dropSession: dropSession,
            getSession: getSession,
            getOs: getOs
        }
    })