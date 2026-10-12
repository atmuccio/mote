import java.nio.ByteBuffer;

class StreamInput {
    private ByteBuffer inBuffer;
    private final Object other = new Object();
    private volatile boolean ready;

    // The block locks a local copy of the field, then waits on the field: the same object.
    int read() throws InterruptedException {
        ByteBuffer copy = inBuffer;
        if (copy == null) {
            return -1;
        }
        synchronized (copy) {
            while (inBuffer.position() == 0) {
                inBuffer.wait(100);
            }
        }
        return 1;
    }

    // The other way round: lock the field, notify through the copy.
    void signal() {
        final ByteBuffer local = this.inBuffer;
        synchronized (inBuffer) {
            local.notifyAll();
        }
    }

    // A copy of another monitor is still the wrong one.
    void wrong() throws InterruptedException {
        Object lock = other;
        synchronized (lock) {
            inBuffer.wait();                                           // expect: wait-notify-wrong-monitor
        }
    }

    // A reassigned copy may hold another object.
    void reassigned(ByteBuffer next) throws InterruptedException {
        ByteBuffer copy = inBuffer;
        copy = next;
        synchronized (copy) {
            inBuffer.wait();                                           // expect: wait-notify-wrong-monitor
        }
    }
}
