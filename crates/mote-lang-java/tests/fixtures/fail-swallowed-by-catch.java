// mote-test: test-file
import static org.junit.Assert.assertTrue;
import static org.junit.Assert.fail;

import org.junit.Assert;
import org.junit.Test;

public class ParserTest {
    @Test
    public void swallowsThrowable() {
        try {
            parse("");
            fail("expected a parse error");                         // expect: fail-swallowed-by-catch
        } catch (Throwable expected) {
        }
    }

    @Test
    public void swallowsError() {
        try {
            parse("");
            Assert.fail();                                          // expect: fail-swallowed-by-catch
        } catch (IllegalArgumentException e) {
            // fine
        } catch (Error e) {
            log(e);
        }
    }

    @Test
    public void swallowsAssertionError() {
        try {
            check("");
            fail();                                                 // expect: fail-swallowed-by-catch
        } catch (AssertionError | RuntimeException e) {
            // expected
        }
    }

    @Test
    public void fine() {
        try {
            parse("");
            fail("expected");
        } catch (IllegalArgumentException expected) {
        }
        try {
            parse("");
            fail("expected");
        } catch (AssertionError e) {
            throw e;
        } catch (Throwable t) {
        }
        try {
            parse("");
            fail("expected");
        } catch (Throwable t) {
            assertTrue(t instanceof IllegalArgumentException);
        }
        try {
            parse("");
            fail("expected");
        } catch (Exception e) {
        }
        try {
            try {
                parse("");
                fail("inner");
            } catch (IllegalStateException e) {
            }
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
        try {
            Runnable r = () -> fail("later");
            r.run();
        } catch (Throwable t) {
        }
        try {
            parse("");
            failover();
        } catch (Throwable t) {
        }
    }

    void parse(String s) { }
    void check(String s) { }
    void log(Throwable t) { }
    void failover() { }
}
