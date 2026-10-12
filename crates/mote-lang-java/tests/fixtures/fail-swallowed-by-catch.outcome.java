// mote-test: test-file
import static org.assertj.core.api.Assertions.assertThat;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.junit.jupiter.api.Assertions.fail;

import java.util.List;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.concurrent.atomic.AtomicReference;
import java.util.function.BooleanSupplier;
import org.junit.jupiter.api.Test;

class OutcomeTest {
    private final List<Throwable> problems = new CopyOnWriteArrayList<>();
    private final AtomicReference<Throwable> firstProblem = new AtomicReference<>();
    private static final org.slf4j.Logger LOG = org.slf4j.LoggerFactory.getLogger(OutcomeTest.class);

    // A polling check: the failed assertion becomes `false`, the helper retries, and the
    // outer assertTrue fails the test on time-out.
    @Test
    void pollsUntilReady() {
        assertTrue(Poller.until(() -> {
            try {
                assertCounts(1, 2);
                return true;
            } catch (AssertionError | RuntimeException e) {
                return false;
            }
        }, 5000));
        assertTrue(Poller.until(new BooleanSupplier() {
            @Override
            public boolean getAsBoolean() {
                try {
                    assertEquals(2, size());
                    return true;
                } catch (AssertionError e) {
                    return false;
                }
            }
        }, 5000));
    }

    // The assertion is the code under test: it is expected to throw.
    static void rejectsLateValue(int value, int limit) {
        try {
            assertThat(value).isLessThan(limit);
        } catch (AssertionError expected) {
            // the failure was expected
            return;
        }
        fail("the assertion should have failed");
    }

    static void rejectsBothForms(int value, int limit) {
        try {
            assertThat(value).isLessThan(limit);
        } catch (AssertionError e) {
            try {
                assertThat(value).isLessThan(Integer.valueOf(limit));
            } catch (AssertionError again) {
                return;
            }
        }
        fail("the assertion should have failed");
    }

    static void rejectsOrThrows(int value) {
        try {
            assertThat(value).isNegative();
        } catch (AssertionError e) {
            return;
        }
        throw new IllegalStateException("expected the assertion to fail");
    }

    // fail() whose failure is recorded for a check after the run.
    void onMessage(long seq, long expected) {
        try {
            if (seq != expected) {
                fail("got " + seq + " instead of " + expected);
            }
        } catch (Throwable t) {
            LOG.error("message handling failed: " + t);
            problems.add(t);
        }
        try {
            if (seq < 0) {
                fail("negative sequence");
            }
        } catch (Throwable t) {
            firstProblem.compareAndSet(null, t);
        }
    }

    // Still swallowed: nothing fails after an early return.
    @Test
    void earlyReturnWithoutFollowUp() {
        try {
            assertEquals(3, size());                       // expect: fail-swallowed-by-catch
        } catch (AssertionError e) {
            return;
        }
    }

    // Still swallowed: both paths give the same result.
    boolean sameResult() {
        try {
            assertEquals(3, size());                       // expect: fail-swallowed-by-catch
            return true;
        } catch (AssertionError e) {
            return true;
        }
    }

    // Still swallowed: fail() itself is caught; the handler only logs it.
    @Test
    void loggedFailure() {
        try {
            size();
            fail("expected an error");                     // expect: fail-swallowed-by-catch
        } catch (Throwable t) {
            LOG.warn("caught", t);
        }
    }

    // Still swallowed: the early return also skips the fail() inside the try.
    @Test
    void failInsideTryWithEarlyReturn() {
        try {
            size();
            fail("expected an error");                     // expect: fail-swallowed-by-catch
        } catch (Throwable t) {
            return;
        }
        fail("unreachable when size() returns");
    }

    void assertCounts(int a, int b) {
        assertEquals(a, b);
    }

    int size() { return 3; }

    static class Poller {
        static boolean until(BooleanSupplier check, long timeoutMillis) {
            return check.getAsBoolean();
        }
    }
}
