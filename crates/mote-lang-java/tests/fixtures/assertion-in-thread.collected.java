// mote-test: test-file
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.junit.jupiter.api.Assertions.fail;

import java.util.List;
import java.util.Vector;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.atomic.AtomicReference;
import org.junit.jupiter.api.Assertions;
import org.junit.jupiter.api.Test;

class WaiterTest {
    private final List<Throwable> problems = new Vector<>();
    private final AtomicReference<Throwable> problem = new AtomicReference<>();

    // The handler records the error in a shared collection that the test checks after join;
    // the fail() after it is redundant but harmless.
    @Test
    void collectsThenFails() throws Exception {
        CountDownLatch done = new CountDownLatch(1);
        Thread waiter = new Thread() {
            @Override
            public void run() {
                try {
                    done.await();
                } catch (InterruptedException e) {
                    problems.add(e);
                    Assertions.fail(e.getMessage());
                }
            }
        };
        Thread other = new Thread(() -> {
            try {
                done.await();
            } catch (InterruptedException e) {
                problem.set(e);
                fail("interrupted");
            }
        });
        waiter.start();
        other.start();
        done.countDown();
        waiter.join();
        other.join();
        assertTrue(problems.isEmpty());
        assertTrue(problem.get() == null);
    }

    // Still lost: the handler only prints, then fails on the other thread.
    @Test
    void printsThenFails() throws Exception {
        CountDownLatch done = new CountDownLatch(1);
        Thread waiter = new Thread(() -> {
            try {
                done.await();
            } catch (InterruptedException e) {
                System.out.println(e);
                fail("interrupted");                                // expect: assertion-in-thread
            }
        });
        waiter.start();
        waiter.join();
    }

    // Still lost: a collected handler does not cover a later assertion outside it.
    @Test
    void collectsButAssertsLater() throws Exception {
        CountDownLatch done = new CountDownLatch(1);
        Thread waiter = new Thread(() -> {
            try {
                done.await();
            } catch (InterruptedException e) {
                problems.add(e);
                fail("interrupted");
            }
            assertTrue(done.getCount() == 0);                       // expect: assertion-in-thread
        });
        waiter.start();
        waiter.join();
    }
}
