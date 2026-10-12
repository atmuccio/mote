// mote-test: test-file
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.assertj.core.api.Assertions.assertThat;

import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import org.junit.jupiter.api.Assertions;
import org.junit.jupiter.api.Test;

class WorkerTest {
    private final ExecutorService executor = Executors.newFixedThreadPool(2);

    @Test
    void plainThread() throws Exception {
        Thread t = new Thread(() -> {
            int v = compute();
            assertEquals(42, v);                                    // expect: assertion-in-thread
        });
        t.start();
        t.join();
    }

    @Test
    void anonymousRunnable() throws Exception {
        Thread t = new Thread(new Runnable() {
            @Override
            public void run() {
                assertTrue(compute() > 0);                          // expect: assertion-in-thread
            }
        });
        t.start();
        t.join();
    }

    @Test
    void threadSubclass() throws Exception {
        Thread t = new Thread() {
            @Override
            public void run() {
                Assertions.assertEquals(1, compute());              // expect: assertion-in-thread
            }
        };
        t.start();
        t.join();
    }

    @Test
    void executeTask() {
        executor.execute(() -> assertThat(compute()).isPositive()); // expect: assertion-in-thread
    }

    @Test
    void discardedSubmit() {
        executor.submit(() -> {
            assertEquals(42, compute());                            // expect: assertion-in-thread
        });
    }

    @Test
    void discardedRunAsync() {
        CompletableFuture.runAsync(() -> assertEquals(1, compute())); // expect: assertion-in-thread
    }

    // ---- compliant ------------------------------------------------------------------------

    @Test
    void futureChecked() throws Exception {
        Future<?> f = executor.submit(() -> assertEquals(42, compute()));
        f.get();
        CompletableFuture.runAsync(() -> assertEquals(1, compute())).join();
    }

    @Test
    void errorsCollected() throws Exception {
        List<Throwable> errors = new ArrayList<>();
        Thread t = new Thread(() -> {
            try {
                assertEquals(42, compute());
            } catch (AssertionError e) {
                errors.add(e);
            }
        });
        t.start();
        t.join();
        assertTrue(errors.isEmpty());
    }

    @Test
    void latchAfterAssertion() throws Exception {
        CountDownLatch done = new CountDownLatch(1);
        executor.execute(() -> {
            assertEquals(42, compute());
            done.countDown();
        });
        assertTrue(done.await(1, java.util.concurrent.TimeUnit.SECONDS));
    }

    @Test
    void noAssertionInTask() throws Exception {
        Thread t = new Thread(() -> compute());
        t.start();
        t.join();
        assertEquals(1, 1);
    }

    @Test
    void notAThread() {
        Runnable r = () -> assertEquals(1, compute());
        r.run();
    }

    int compute() {
        return 42;
    }
}
