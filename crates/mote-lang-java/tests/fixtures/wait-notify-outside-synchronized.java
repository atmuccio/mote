class Monitor {
    private boolean ready;

    void awaitReady() throws InterruptedException {
        while (!ready) {
            wait();                                                   // expect: wait-notify-outside-synchronized
        }
    }

    void signal() {
        ready = true;
        notifyAll();                                                  // expect: wait-notify-outside-synchronized
        this.notify();                                                // expect: wait-notify-outside-synchronized
    }

    void timed() throws InterruptedException {
        this.wait(100L);                                              // expect: wait-notify-outside-synchronized
    }

    Monitor() {
        notifyAll();                                                  // expect: wait-notify-outside-synchronized
    }

    void nestedAnonymous() {
        synchronized (this) {
            new Thread() {
                public void run() {
                    notifyAll();                                      // expect: wait-notify-outside-synchronized
                }
            }.start();
        }
    }

    // negatives
    synchronized void guarded() throws InterruptedException {
        while (!ready) {
            wait();
        }
        notifyAll();
    }

    void block() throws InterruptedException {
        synchronized (this) {
            if (!ready) {
                wait(10, 0);
            }
            notifyAll();
        }
    }

    void otherLock(Object lock) throws InterruptedException {
        synchronized (lock) {
            lock.wait();
            lock.notifyAll();
        }
        lock.notify();                                               // qualified: another object
    }

    void lambda() {
        Runnable r = () -> notifyAll();                              // unknown calling context
    }

    void condition(java.util.concurrent.locks.Condition c) throws InterruptedException {
        c.await();
        c.signalAll();
    }

    void withArgs() {
        notifyAll(1);
    }
}

class OwnWait {
    void wait(int seconds) { }
    void notify(String event) { }

    void m() {
        wait(5);                                                     // own overload
        notify();                                                    // class declares a notify overload
    }
}
