// mote-test: test-file
import static org.junit.Assert.assertEquals;

import org.junit.Test;

// JUnit 4: parameterization is per class (a runner), not per method.
public class LegacyNumbersTest {
    @Test public void one() { assertEquals(1, parse("one")); }
    @Test public void two() { assertEquals(2, parse("two")); }
    @Test public void three() { assertEquals(3, parse("three")); }

    static int parse(String s) { return 0; }
}
