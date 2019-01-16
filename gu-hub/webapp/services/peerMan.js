angular.module('gu').service('guPeerMan', function ($http, $log, $q, hubApi) {

    const CREATE = 37;
    const UPDATE = 38;
    const GET_SESSIONS = 39;
    const DESTROY = 40;

    class PeerManager {
        constructor(session, nodeId) {
            $log.info('new-session', session, 'node-id', nodeId);
            this.session = session;
            this.nodeId = nodeId;
        }

        newSession(sessionSpec) {
            return new Session(this.nodeId, hubApi.callRemote(this.nodeId, CREATE, sessionSpec));
        }

        fromId(sessionId, sessionData) {
            return new Session(this.nodeId, {Ok: sessionId}, sessionData);
        }

        sessions() {
            return hubApi.callRemote(this.nodeId, GET_SESSIONS, {})
                .then(sessions => {
                    var sessions = _.map(sessions.Ok, session => this.fromId(session.id, session));
                    return sessions;
                })
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
                }
                else {
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
                        {exec: {executable: entry, args: (args||[])}}
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
                        {start: {executable: entry, args: (args||[])}},
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

            return hubApi.callRemote(this.nodeId, DESTROY, {session_id : this.id}).then(result => {
                if (result.Ok) {
                    $log.info("session", this.id, "closed: ", result);
                } else {
                    $log.error("session", this.id, "closing error:", result);
                }
                return result;
            });
        }
    }

    function peer(session, nodeId) {
        return new PeerManager(session, nodeId);
    }

    return { peer: peer }

});

