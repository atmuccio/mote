import java.util.List;
import java.util.Queue;

class Blocks {
    static {} // expect: empty-block

    {} // expect: empty-block

    private final Object lock = new Object();

    void positives(boolean ok, List<String> items) {
        if (ok) {} else { reload(); } // expect: empty-block
        if (ok) {
            reload();
        } else {} // expect: empty-block
        if (ok) {
            reload();
        } else if (items.isEmpty()) {} // expect: empty-block
        for (String s : items) {} // expect: empty-block
        for (int i = 0; i < 10; i++) {} // expect: empty-block
        while (ok) {} // expect: empty-block
        do {} while (ok); // expect: empty-block
        try {} finally { reload(); } // expect: empty-block
        try { reload(); } finally {} // expect: empty-block
        synchronized (lock) {} // expect: empty-block
        switch (items.size()) {} // expect: empty-block
        {} // expect: empty-block
        switch (items.size()) {
            case 1: {} // expect: empty-block
                break;
            default:
                reload();
        }
    }

    Blocks() {
        {} // expect: empty-block
    }

    void negatives(boolean ok, Queue<String> queue, int[] a, List<String> items) {
        if (ok) {
            // intentionally nothing: the default is fine
        }
        while (queue.poll() != null) {}          // drains the queue in the header
        int i;
        for (i = 0; a[i] != 0; i++) {}            // search loop: the counter is the result
        for (;;) { if (ok) break; }
        try { reload(); } catch (RuntimeException e) {}   // catch blocks: empty-catch-block
        try { } catch (RuntimeException e) { }            // no-op statement: left to empty-catch-block
        try {} catch (RuntimeException e) { reload(); } // expect: empty-block
        Runnable r = () -> {};                    // lambda bodies are fine
        switch (items.size()) {
            case 1 -> {}
            default -> reload();
        }
        try (AutoCloseable c = open()) {} catch (Exception e) { reload(); }
        synchronized (lock) {
            /* memory barrier only */
        }
        Object o = new Object() {
            @Override public String toString() { return ""; }
        };
    }

    void emptyMethod() {}

    interface Callback { default void call() {} }

    enum Mode {
        A, B;
        {}  // expect: empty-block
    }

    record R(int x) {
        R {}
    }

    AutoCloseable open() { return null; }
    void reload() {}
}
