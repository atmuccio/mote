import java.util.concurrent.locks.Condition;
import java.util.concurrent.locks.Lock;
import java.util.concurrent.locks.ReadWriteLock;
import java.util.concurrent.locks.ReentrantLock;
import java.util.concurrent.locks.ReentrantReadWriteLock;

class Account {
    private final ReentrantLock lock = new ReentrantLock();
    private final Condition notEmpty = lock.newCondition();
    private final ReadWriteLock rw = new ReentrantReadWriteLock();
    private final Object monitor = new Object();
    private long balance;

    void wrong(long amount, Lock other) throws InterruptedException {
        synchronized (lock) {                                   // expect: intrinsic-lock-on-lock-object
            balance += amount;
        }
        synchronized (rw) {                                     // expect: intrinsic-lock-on-lock-object
            balance -= amount;
        }
        synchronized (other) {                                  // expect: intrinsic-lock-on-lock-object
            notEmpty.notify();                                  // expect: intrinsic-lock-on-lock-object
        }
        notEmpty.wait();                                        // expect: intrinsic-lock-on-lock-object
        lock.notifyAll();                                       // expect: intrinsic-lock-on-lock-object
    }

    void fine(long amount, Holder h) throws InterruptedException {
        lock.lock();
        try {
            balance += amount;
            notEmpty.signal();
            notEmpty.await();
        } finally {
            lock.unlock();
        }
        synchronized (monitor) {
            monitor.notifyAll();
            monitor.wait();
        }
        synchronized (this) {
            wait(10);
        }
        synchronized (h.lock()) {
            h.notifyAll();
        }
    }

    interface Holder { Object lock(); }
}
