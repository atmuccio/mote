// mote-test: test-file
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.TestFactory;
import org.junit.jupiter.params.ParameterizedTest;
import java.util.stream.Stream;

class ParserTest {
    @Test
    private void parsesDates() { }                                  // expect: test-method-not-run

    @Test
    static void parsesTimes() { }                                   // expect: test-method-not-run

    @Test
    int computesTotal() { return 1; }                               // expect: test-method-not-run

    @ParameterizedTest
    private void parameterized(int x) { }                           // expect: test-method-not-run

    @org.junit.jupiter.api.Test
    public String qualified() { return ""; }                        // expect: test-method-not-run

    // Compliant
    @Test
    void parsesNumbers() { }

    @Test
    public void publicOne() { }

    @TestFactory
    Stream<Object> factory() { return Stream.empty(); }

    private int helper() { return 2; }

    static void util() { }

    @Deprecated
    private void notATest() { }
}
