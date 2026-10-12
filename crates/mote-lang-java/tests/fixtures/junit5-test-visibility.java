// mote-test: test-file
import org.junit.jupiter.api.AfterAll;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Nested;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.params.ParameterizedTest;
import org.junit.jupiter.params.provider.ValueSource;

public class InvoiceTest {                                          // expect: junit5-test-visibility
    @BeforeEach
    public void setUp() { }                                         // expect: junit5-test-visibility

    @Test
    public void computesTotal() { }                                 // expect: junit5-test-visibility

    @ParameterizedTest
    @ValueSource(ints = {1, 2})
    public void acceptsQuantities(int q) { }                        // expect: junit5-test-visibility

    @AfterAll
    public static void tearDownAll() { }                            // expect: junit5-test-visibility

    @Test
    void alreadyPackagePrivate() { }

    public InvoiceTest() { }

    private void helper() { }

    @Nested
    public class WhenEmpty {                                        // expect: junit5-test-visibility
        @Test
        public void hasNoLines() { }                                // expect: junit5-test-visibility
    }
}

public class SharedFixturesTest {
    public static final String SAMPLE = "x";                        // used by other tests: keep public class

    @Test
    public void works() { }                                         // expect: junit5-test-visibility

    public void notATest() { }
}

public abstract class BaseContractTest {
    @Test
    public void contract() { }                                      // expect: junit5-test-visibility
}

interface TestLifecycleLogger {
    @BeforeEach
    default void logStart() { }
}

class Overrides extends BaseContractTest {
    @Override
    @Test
    public void contract() { }                                      // overrides: keep

    @BeforeEach
    public void setUp() { }                                         // may override a public base hook

    @Test
    public void own() { }                                           // expect: junit5-test-visibility
}

public class NoTests {
    public void helper() { }
}
