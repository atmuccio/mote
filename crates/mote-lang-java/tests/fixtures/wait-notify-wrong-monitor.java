class Monitors {
    private final Object readyLock = new Object();
    private final Object other = new Object();
    private boolean ready;

    synchronized void awaitReady() throws InterruptedException {
        while (!ready) {
            readyLock.wait();                           // expect: wait-notify-wrong-monitor
        }
    }

    void signal() {
        synchronized (other) {
            readyLock.notifyAll();                      // expect: wait-notify-wrong-monitor
            this.readyLock.notify();                    // expect: wait-notify-wrong-monitor
        }
        synchronized (this) {
            readyLock.wait(100L);                       // expect: wait-notify-wrong-monitor
        }
    }

    class Inner {
        synchronized void poke() {
            Monitors.this.notify();                     // expect: wait-notify-wrong-monitor
        }
    }

    // Negatives.
    void ok() throws InterruptedException {
        synchronized (readyLock) {
            while (!ready) {
                readyLock.wait();
            }
            this.readyLock.notifyAll();
        }
        synchronized (this.readyLock) {
            readyLock.notify();
        }
        synchronized (other) {
            synchronized (readyLock) {
                readyLock.wait(10);
            }
        }
        synchronized (lockFor("x")) {
            readyLock.notify();                         // lock expression not comparable
        }
    }

    synchronized void onThis() throws InterruptedException {
        wait();
        this.notifyAll();
    }

    void helper() {
        readyLock.notifyAll();                          // no synchronized context: callers may hold it
    }

    synchronized void lambdas() {
        Runnable r = () -> {
            synchronized (readyLock) {
                readyLock.notify();
            }
        };
        Runnable s = () -> readyLock.notify();          // context unknown
    }

    synchronized void overload(Waiter w) {
        w.wait("name");                                 // not Object.wait
    }

    interface Waiter { void wait(String s); }

    Object lockFor(String s) { return readyLock; }
}

class CallerHoldsLock {
    private final Object stateLock = new Object();
    private final Object control = new Object();

    protected void checkState() {
        assert Thread.holdsLock(stateLock);
        synchronized (control) {
            stateLock.notifyAll();
        }
    }
}
