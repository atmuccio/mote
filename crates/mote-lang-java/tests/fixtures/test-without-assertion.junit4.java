// mote-test: test-file
import org.junit.Ignore;
import org.junit.Rule;
import org.junit.Test;
import org.junit.rules.ExpectedException;

import static org.junit.Assert.assertEquals;

public class LegacyTest {
    @Rule
    public ExpectedException thrown = ExpectedException.none();

    @Test
    public void noAssert() {                                        // expect: test-without-assertion
        new Legacy().run();
    }

    @Test(timeout = 1000)
    public void timeoutOnly() {                                     // expect: test-without-assertion
        new Legacy().run();
    }

    @org.junit.Test
    public void qualifiedAnnotation() {                             // expect: test-without-assertion
        new Legacy().run();
    }

    // negatives
    @Test(expected = IllegalStateException.class)
    public void expectedException() {
        new Legacy().fail();
    }

    @Test
    public void ruleExpect() {
        thrown.expect(IllegalStateException.class);
        new Legacy().fail();
    }

    @Test
    public void assertion() {
        assertEquals(1, new Legacy().run());
    }

    @Ignore
    @Test
    public void ignored() {
        new Legacy().run();
    }

    @Test
    public abstract void abstractTest();
}
