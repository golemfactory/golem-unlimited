angular.module('gu').service('sessionMan', function ($http, $log, $q, hubApi, guPeerMan) {

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

    function create(sessionType, env) {
        var session = {
            id: guid(),
            type: sessionType,
            env: env,
            status: 'NEW'
        };
        sessions.push(session);
        save();

        return session;
    }

    function send(node_id) {
        return function (destination, body) {
            return $http.post('/peers/send-to', [node_id, destination, body]).then(r => r.data
            )
                ;
        }
    }

    function peers(session, needDetails) {
        var peersPromise;

        if (session && session.status !== 'NEW' && session.peers) {
            peersPromise = $q.when(_.map(session.peers, peer => angular.copy(peer))
            )
            ;
        }
        else {
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
        return _.find(sessions, session => session.id === sessionId
        )
            ;
    }

    function getOs(nodeId) {
        if (nodeId in osMap) {
            return $q.when(osMap[nodeId]);
        }
        else {
            return hubApi.callRemote(nodeId, 19354, null).then(data => {
                var ok = data.ok;
                if (ok) {
                    osMap[peer.nodeId] = ok.os || peer.os || 'unk';
                }
                return osMap[peer.nodeId];
            });
        }
    }

    return {
        create: create,
        peers: peers,
        sessions: listSessions,
        peerDetails: peerDetails,
        updateSession: updateSession,
        dropSession: dropSession,
        getSession: getSession,
        getOs: getOs
    }
})