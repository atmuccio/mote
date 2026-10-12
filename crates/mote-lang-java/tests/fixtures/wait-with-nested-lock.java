import java.util.ArrayDeque;

class Pool {
    private final Object registry = new Object();
    private final ArrayDeque<Runnable> queue = new ArrayDeque<>();

    void take() throws InterruptedException {
        synchronized (registry) {
            synchronized (queue) {
                while (queue.isEmpty()) {
                    queue.wait();                               // expect: wait-with-nested-lock
                }
            }
        }
    }

    void put(Runnable r) {
        synchronized (registry) {
            synchronized (queue) {
                queue.add(r);
                queue.notifyAll();
            }
        }
    }

    synchronized void await() throws InterruptedException {
        synchronized (this.queue) {
            queue.wait(100L);                                   // expect: wait-with-nested-lock
        }
    }

    synchronized void signal() {
        synchronized (queue) {
            queue.notify();
        }
    }
}

class Store {
    private final Object closedLock = new Object();
    private final Object inprogressLock = new Object();
    private int inprogress;

    void sync() throws InterruptedException {
        synchronized (closedLock) {                 // keeps new work out while in-flight work drains
            synchronized (inprogressLock) {
                while (inprogress > 0) {
                    inprogressLock.wait();
                }
            }
        }
    }

    void done() {
        synchronized (inprogressLock) {             // the notifier does not need closedLock
            inprogress--;
            inprogressLock.notifyAll();
        }
    }

    void single() throws InterruptedException {
        synchronized (inprogressLock) {
            inprogressLock.wait();
        }
    }

    void other(Waiter w) {
        synchronized (closedLock) {
            synchronized (inprogressLock) {
                w.wait("x");
            }
        }
    }

    interface Waiter { void wait(String s); }
}
