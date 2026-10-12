import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.locks.Condition;
import java.util.concurrent.locks.Lock;
import java.util.concurrent.locks.ReentrantLock;

class Buffer {
    private final Lock lock = new ReentrantLock();
    private final Condition notEmpty = lock.newCondition();
    private final CountDownLatch started = new CountDownLatch(1);
    private int count;

    Object take() throws InterruptedException {
        lock.lock();
        try {
            if (count == 0) {
                notEmpty.await();                       // expect: wait-without-loop
            }
            return next();
        } finally {
            lock.unlock();
        }
    }

    Object takeQuietly() {
        lock.lock();
        try {
            if (count == 0) {
                notEmpty.awaitUninterruptibly();        // expect: wait-without-loop
            }
            return next();
        } finally {
            lock.unlock();
        }
    }

    Object takeLooped() throws InterruptedException {
        lock.lock();
        try {
            while (count == 0) {
                notEmpty.await();
            }
            return next();
        } finally {
            lock.unlock();
        }
    }

    boolean bounded() throws InterruptedException {
        lock.lock();
        try {
            return notEmpty.await(1, TimeUnit.SECONDS);
        } finally {
            lock.unlock();
        }
    }

    void startGate(Waiter w) throws InterruptedException {
        started.await();                                // a latch waits for good
        w.await();                                      // unknown type
    }

    interface Waiter { void await(); }

    Object next() { return null; }
}
