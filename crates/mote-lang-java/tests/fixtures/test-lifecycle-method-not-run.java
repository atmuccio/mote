// mote-test: test-file
import org.junit.jupiter.api.AfterAll;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Nested;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.TestInstance;

class ParserTest {
    @BeforeAll                                  // expect: test-lifecycle-method-not-run
    void startServer() {}

    @AfterAll
    static void stopServer() {}

    @BeforeEach                                 // expect: test-lifecycle-method-not-run
    static void resetStatic() {}

    @AfterEach                                  // expect: test-lifecycle-method-not-run
    private void cleanUp() {}

    @BeforeEach                                 // expect: test-lifecycle-method-not-run
    int prepare() { return 1; }

    @BeforeEach
    void setUp() {}

    @Test
    void parses() {}

    @Nested
    @TestInstance(TestInstance.Lifecycle.PER_CLASS)
    class PerClass {
        @BeforeAll
        void once() {}

        @Test
        void works() {}
    }
}

@TestInstance(TestInstance.Lifecycle.PER_CLASS)
class SharedStateTest {
    @BeforeAll
    void once() {}

    @Nested
    class Inner {
        @BeforeAll
        void alsoOnce() {}
    }
}

class DerivedTest extends BaseTest {
    @BeforeAll
    void maybePerClass() {}                     // the base class may carry @TestInstance
}

class BaseTest {}

class OverridingTest extends BaseTest {
    @Override
    @BeforeEach
    public void hook() {}
}
