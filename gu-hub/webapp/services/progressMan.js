
angular.module('gu').service('guProgressMan',function($interval, $q, $log) {

    class GuProgressDomain {

        constructor(total) {
            this.total = total || 0;
            this.count = 0;
            this.subTasks = new Map();
        }

        createTask(tag, label, total) {
            return new GuTask(this, tag, label, total);
        }

        createAutoTask(tag, label, estimate, donePromise) {
            return new GuAutoTask(this, tag, label, estimate, donePromise);
        }

        getTask(tag) {
            if (this.subTasks.has(tag)) {
                return this.subTasks.get(tag);
            }
            return this.createTask(tag, '', 0);
        }

        toSon() {
            let tasks = [];

            for (let task of this.subTasks.values()) {
                tasks.push(task.toSon());
            }

            let son = {
                tag: this.tag,
                total: this.total,
                count: this.count,
            };

            if (this.label) {
                son.label = this.label;
            }

            if (tasks) {
                son.tasks = tasks;
            }

            return son;
        }

        toString() {
            return JSON.stringify(this.toSon());
        }


        isCompleted() {
            return Math.abs(this.total - this.count)/Math.max(this.total, 1) < 0.001;
        }

        addTotal(diff) {
            this.total += diff;
        }

        addCount(diff) {
            let oldCount = this.count;
            this.count += diff;
            if (this.count < 0) {
                this.count = 0;
            }
            else if (this.count > this.total) {
                this.count = this.total;
            }

            return this.count - oldCount;
        }

        addTask(tag, task) {
            let prev = this.subTasks.get(tag);

            this.addTotal(task.total - (prev? prev.total:0));
            this.addCount(task.count - (prev? prev.count:0));
            this.subTasks.set(tag, task);
        }

        leadingTask(recursive = false) {

            $log.info('leading', recursive, this.subTasks.size);
            if (this.subTasks.size === 0) {
                return this;
            }

            if (this.$leading && !this.$leading.isCompleted()) {
                return this.$leading;
            }

            for (let task of this.subTasks.values()) {
                if (!task.isCompleted()) {
                    if (recursive) {
                        task = task.leadingTask(recursive);
                    }
                    this.$leading = task;
                    return task;
                }
            }
        }

        leadingTaskFor(tag) {
            let task = this.subTasks[tag];
            if (task) {
                return task.leadingTask();
            }
        }

    }

    class GuTask extends GuProgressDomain {

        constructor(parent, tag, label, total) {
            super(total);
            this.parent = parent;
            this.tag = tag;
            this.label = label;

            parent.addTask(tag, this);
        }


        addTotal(diff) {
            super.addTotal(diff);
            this.parent.addTotal(diff);
        }


        addCount(diff) {
            let nd = super.addCount(diff);
            this.parent.addCount(nd);
        }
    }

    class GuAutoTask extends GuTask {

        constructor(parent, tag, label, total, donePromise) {
            super(parent, tag, label, total);
            this.startTs = currentTime();
            this.endTs = currentTime() + total * 1000 + 500;
            this.closed = false;
            addTimer(this);

            $q.when(donePromise).finally(() => this.close())
        }

        tick(ts) {
            if (this.closed) {
                return false;
            }

            if (ts <= this.endTs) {
                let newCount = (ts - this.startTs) / 1000.0
                this.addCount(newCount - this.count);
                return !this.closed;
            }
            // pos = (ts-barrier.start)*barrier.mult + barrier.base;

            if (!this.barrier) {
                let base = this.total;
                let total = base * 1.3;
                let rest = total - base;
                let mult = 0.5;
                let mulPoint = base + rest*0.5;
                let start = ts;

                this.barrier = {
                    start: start,
                    mult: mult,
                    mulPoint: mulPoint,
                    base: base,
                };
                this.addTotal(total - this.total);
            }
            let barrier = this.barrier;
            let pos = (ts - barrier.start) * barrier.mult + barrier.base;
            while (pos > barrier.mulPoint) {
                let mulTs = barrier.start + (barrier.mulPoint - barrier.base) / barrier.mult;
                let nextMulPoint = (this.total + barrier.mulPoint)*0.5;

                barrier.start = mulTs;
                barrier.mult = barrier.mult*0.5;
                barrier.base = barrier.mulPoint;
                barrier.mulPoint = nextMulPoint;

                pos = (ts - barrier.start) * barrier.mult + barrier.base;
            }
            this.addCount(pos - this.count);
            return !this.closed;
        }

        close() {
            this.closed = true;
            this.addCount(this.total - this.count);
        }

    }

    function currentTime() {
        let ts = new Date();

        return ts.getTime();
    }

    let timers = [];
    let intervalHandle = null;

    function addTimer(timer) {
        timers.push(timer);
        if (intervalHandle === null) {
            intervalHandle = $interval(tick, 200);
        }
    }

    function tick() {
        let newTimers = [];
        let ts = currentTime();
        for (let timer of timers) {
            if (timer.tick(ts)) {
                newTimers.push(timer);
            }
        }
        timers = newTimers;
        if (timers.length === 0 && intervalHandle !== null) {
            console.log('h', intervalHandle);
            $interval.cancel(intervalHandle);
            //$q.cancel(intervalHandle);
            intervalHandle = null;
        }
    }


    return function () {
        return new GuProgressDomain();
    }
});