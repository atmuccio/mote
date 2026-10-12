// mote-test: test-file
import org.junit.Before;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

class MigratedTest {
    @Before                                     // expect: test-lifecycle-method-not-run
    public void setUp() {}

    @Test
    void parsesEmptyInput() {}
}

class Junit4TestsWithJupiterHook {
    @BeforeEach                                 // expect: test-lifecycle-method-not-run
    void setUp() {}

    @org.junit.Test
    public void legacy() {}
}

class HalfMigratedTest {
    @Before
    public void setUpLegacy() {}

    @BeforeEach
    void setUp() {}

    @Test
    void modern() {}

    @org.junit.Test
    public void legacy() {}
}

class SubclassTest extends Object {
    @Before
    public void setUp() {}                      // inherited JUnit 4 tests may use it

    @Test
    void modern() {}
}
