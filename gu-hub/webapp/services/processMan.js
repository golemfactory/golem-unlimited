
angular.module('gu').service('guProcessMan', function ($http, $log, $q, hubApi, guPeerMan) {
    "use strict";


    class Process {

        createWork() {

        }

        addResult(work, result) {

        }

        get progress() {
            return 0;
        }

        get isActive() {
            return true;
        }

        stop() {

        }

    }

    class ProcessManager {
        constructor(hubSession) {
            this.hubSession = hubSession;
            this._process = undefined;
            this._counters = {};
        }

        get process() {
            return this._process;
        }

        get progress() {
            return this._process ? this._process.progress : 100;
        }

        get isActive() {
            return this._process && this._process.isActive;
        }

        _incCnt(nodeId) {
            if (nodeId in this._counters) {
                this._counters[nodeId]+=1;
            }
            else {
                this._counters[nodeId] = 1;
            }
        }

        workCnt(nodeId) {
            return this._counters[nodeId] || 0;
        }

        run(process) {
            if (this._process == process) {
                return;
            }
            if (this._process) {
                let oldProcess = this._process;
                this._process = undefined;
                oldProcess.stop();
            }
            this._process = process;

            let self = this;
            async function processWork(deployment) {
                if (self._process !== process) {
                    return;
                }

                let work = await process.createWork();

                while (work && self._process === process) {
                    let result = await deployment.update(work.commands);
                    self._incCnt(deployment.node.id);

                    if (self._process === process) {
                        self.addResult(work, result);
                    }

                    work = self.produceWork();
                }
            }


        }

    }


});
