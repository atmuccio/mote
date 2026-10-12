import java.util.ArrayList;
import java.util.List;

class Mailbox {
    private final List<String> items = new ArrayList<>();
    private final Object lock = new Object();
    private boolean ready;

    synchronized String take() throws InterruptedException {
        if (items.isEmpty()) {
            wait();                                                 // expect: wait-without-loop
        }
        return items.remove(0);
    }

    String poll(long timeout) throws InterruptedException {
        synchronized (lock) {
            lock.wait(timeout);                                     // timed: a bounded wait
        }
        synchronized (this) {
            this.wait();                                            // expect: wait-without-loop
        }
        return null;
    }

    void inLambdaInsideLoop() {
        while (!ready) {
            Runnable r = () -> {
                try {
                    synchronized (lock) { lock.wait(); }            // expect: wait-without-loop
                } catch (InterruptedException e) { }
            };
        }
    }

    // Compliant
    synchronized String takeLoop() throws InterruptedException {
        while (items.isEmpty()) {
            wait();
        }
        return items.remove(0);
    }

    synchronized void doLoop() throws InterruptedException {
        do {
            wait(100);
        } while (!ready);
    }

    synchronized void forLoop() throws InterruptedException {
        for (;;) {
            if (ready) break;
            wait();
        }
    }

    void waitOn(Waiter w) {
        w.wait("x");                                                 // someone's own wait(String)
    }

    interface Waiter { void wait(String s); }
}

class OwnWait {
    void wait(int seconds) { }

    void use() {
        wait(5);                                                    // this class's own overload
    }
}
