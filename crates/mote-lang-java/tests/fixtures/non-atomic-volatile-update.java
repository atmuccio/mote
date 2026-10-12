import java.util.concurrent.atomic.AtomicInteger;

class Counters {
    private volatile int requests;
    private volatile long bytes;
    private volatile boolean flag;
    private volatile String log = "";
    private int plain;
    private final AtomicInteger atomic = new AtomicInteger();

    Counters() {
        requests++;                                                 // constructor: not shared yet
        this.bytes += 10;
    }

    void onRequest(int size) {
        requests++;                                                 // expect: non-atomic-volatile-update
        --requests;                                                 // expect: non-atomic-volatile-update
        this.bytes += size;                                         // expect: non-atomic-volatile-update
        bytes -= 1;                                                 // expect: non-atomic-volatile-update
        flag ^= true;                                               // expect: non-atomic-volatile-update
        log += "x";                                                 // expect: non-atomic-volatile-update
        Runnable r = () -> requests++;                              // expect: non-atomic-volatile-update
        new Thread(new Runnable() {
            public void run() { requests++; }                       // expect: non-atomic-volatile-update
        }).start();
    }

    // Compliant
    void fine(int requests) {
        requests++;                                                 // parameter shadows the field
        plain++;
        atomic.incrementAndGet();
        this.flag = true;
        bytes = 0;
        int local = 0;
        local += 2;
    }

    synchronized void guardedMethod() {
        requests++;
    }

    void guardedBlock() {
        synchronized (this) {
            bytes += 2;
        }
    }

    static class Other {
        int requests;
        void bump() { requests++; }                                 // different, non-volatile field
    }
}
