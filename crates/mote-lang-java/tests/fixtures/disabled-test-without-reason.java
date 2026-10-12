// mote-test: test-file
import org.junit.jupiter.api.Disabled;
import org.junit.jupiter.api.Test;

class ImportTest {
    @Disabled                                                       // expect: disabled-test-without-reason
    @Test
    void importsLargeFile() { }

    @Disabled("")                                                   // expect: disabled-test-without-reason
    @Test
    void blank() { }

    @Disabled(value = "   ")                                        // expect: disabled-test-without-reason
    @Test
    void whitespace() { }

    @Disabled()                                                     // expect: disabled-test-without-reason
    @Test
    void emptyParens() { }

    @Test @Disabled void sameLine() { }                             // expect: disabled-test-without-reason

    // negatives
    @Disabled("Flaky on Windows CI, see #1234")
    @Test
    void withReason() { }

    @Disabled(value = "waiting for upstream fix")
    @Test
    void namedValue() { }

    @Disabled(Reasons.FLAKY)
    @Test
    void constantReason() { }

    @Disabled("part one, " + "part two")
    @Test
    void concatenated() { }

    @Test
    void enabled() { }
}

@Disabled                                                           // expect: disabled-test-without-reason
class WholeClassTest { }
