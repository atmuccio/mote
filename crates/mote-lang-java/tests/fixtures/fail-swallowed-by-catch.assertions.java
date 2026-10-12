// mote-test: test-file
import static org.assertj.core.api.Assertions.assertThat;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.util.ArrayList;
import java.util.List;
import org.junit.jupiter.api.Test;

class ParserTest {
    private final List<Throwable> errors = new ArrayList<>();
    private Throwable failure;

    @Test
    void swallowed() {
        try {
            int n = parse("3");
            assertEquals(3, n);                              // expect: fail-swallowed-by-catch
        } catch (Throwable t) {
            System.out.println("parse failed: " + t);
        }
        try {
            assertThat(parse("4")).isEqualTo(4);              // expect: fail-swallowed-by-catch
        } catch (AssertionError e) {
            e.printStackTrace();
        }
        try {
            assertTrue(parse("5") > 0);                       // expect: fail-swallowed-by-catch
        } catch (Error e) {
            // ignore
        }
    }

    @Test
    void notSwallowed() throws Exception {
        try {
            assertEquals(3, parse("3"));
        } catch (Exception e) {                              // an AssertionError is not an Exception
            System.out.println("x");
        }
        try {
            assertEquals(3, parse("3"));
        } catch (Throwable t) {
            errors.add(t);                                   // kept for a later check
        }
        try {
            assertEquals(3, parse("3"));
        } catch (Throwable t) {
            failure = t;
        }
        try {
            assertEquals(3, parse("3"));
        } catch (AssertionError e) {
            throw new IllegalStateException("with context", e);
        }
        try {
            assertEquals(3, parse("3"));
        } catch (Throwable t) {
            assertTrue(t instanceof IllegalStateException);  // asserts on the caught error
        }
        Runnable r = () -> {
            try {
                parse("x");
            } catch (Throwable t) {
                System.out.println(t);
            }
        };
        try {
            helper.assertValid(parse("1"));                  // not a library assertion
        } catch (Throwable t) {
            System.out.println(t);
        }
        assertTrue(errors.isEmpty());
    }

    Helper helper = new Helper();
    static class Helper { void assertValid(int x) {} }

    int parse(String s) { return Integer.parseInt(s); }
}

class CollectingTest {
    @Test
    void collects() {
        StringBuilder failures = new StringBuilder();
        int failureCount = 0;
        for (int i = 0; i < 3; i++) {
            try {
                assertEquals(i, i * 1);
            } catch (Throwable e) {
                failures.append(e.getMessage());
            }
            try {
                assertEquals(i, i * 1);
            } catch (Throwable e) {
                failureCount++;
            }
        }
        assertEquals(0, failures.length() + failureCount);
    }
}

class RetryTest {
    @Test
    void converges() throws Exception {
        for (int i = 0; i < 50; i++) {
            try {
                assertEquals(3, poll());
                return;
            } catch (AssertionError e) {
                // not yet: retry
            }
        }
        org.junit.jupiter.api.Assertions.fail("never converged");
    }

    int poll() { return 3; }
}
