// mote-test: test-file
import static java.lang.Thread.sleep;

import java.util.concurrent.TimeUnit;
import org.junit.jupiter.api.Test;

class PollingTest {
    @Test
    void waits() throws Exception {
        service.start();
        Thread.sleep(2000);                                         // expect: sleep-in-test
        java.lang.Thread.sleep(10);                                 // expect: sleep-in-test
        sleep(5);                                                   // expect: sleep-in-test
        TimeUnit.SECONDS.sleep(1);                                  // expect: sleep-in-test
        Runnable r = () -> {
            try { Thread.sleep(1); } catch (InterruptedException e) { } // expect: sleep-in-test
        };
    }

    void negatives(Clock clock, Sleeper sleeper) throws Exception {
        clock.sleep(10);                                            // unknown type
        sleeper.sleep();
        latch.await(5, TimeUnit.SECONDS);
        Thread.yield();
    }
}
