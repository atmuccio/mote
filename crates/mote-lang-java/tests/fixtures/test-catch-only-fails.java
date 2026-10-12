// mote-test: test-file
import static org.junit.jupiter.api.Assertions.fail;

import java.io.IOException;
import org.junit.Assert;
import org.junit.jupiter.api.Test;

class ParserTest {
    @Test
    void parses() {
        try {
            parse("x");
        } catch (IOException e) {
            fail("unexpected: " + e.getMessage());                   // expect: test-catch-only-fails
        }
    }

    @Test
    void qualified() {
        try {
            parse("y");
        } catch (Exception e) {
            Assert.fail();                                           // expect: test-catch-only-fails
        }
    }

    // ---- compliant ------------------------------------------------------------------------

    @Test
    void expectedException() {
        try {
            parse("");
            fail("should have thrown");
        } catch (IOException expected) {
            // expected
        }
    }

    @Test
    void moreThanFail() {
        try {
            parse("z");
        } catch (IOException e) {
            log(e);
            fail("unexpected");
        }
    }

    @Test
    void inLambda() {
        Runnable r = () -> {
            try {
                parse("w");
            } catch (IOException e) {
                fail(e);
            }
        };
        r.run();
    }

    @Test
    void inAnonymousClass() {
        Runnable r = new Runnable() {
            public void run() {
                try {
                    parse("v");
                } catch (IOException e) {
                    fail(e);
                }
            }
        };
        r.run();
    }

    static class Worker implements Runnable {
        @Override
        public void run() {
            try {
                parse("u");
            } catch (IOException e) {
                fail(e);
            }
        }
    }

    static void parse(String s) throws IOException {}

    static void log(Exception e) {}
}

class ContextTest {
    @Test
    void addsSeed() {
        long seed = 42;
        try {
            check(seed);
        } catch (AssertionError e) {
            fail("seed = " + seed + ": " + e.getMessage());
        }
        try {
            recurse();
        } catch (StackOverflowError e) {
            fail(e.toString());
        }
        try {
            check(seed);
        } catch (Throwable t) {
            fail(t.getMessage());
        }
    }

    void check(long s) {}

    void recurse() {}
}

class CauseKeptTest {
    @Test
    void keepsCause() {
        try {
            ParserTest.parse("q");
        } catch (IOException e) {
            fail("unexpected", e);
        }
    }
}
