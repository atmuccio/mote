// mote-test: test-file
import org.junit.After;
import org.junit.AfterClass;
import org.junit.Before;
import org.junit.BeforeClass;
import org.junit.Test;

public class LegacyTest {
    @BeforeClass                                // expect: test-lifecycle-method-not-run
    public void loadFixtures() {}

    @AfterClass
    public static void unload() {}

    @Before                                     // expect: test-lifecycle-method-not-run
    void notPublic() {}

    @After                                      // expect: test-lifecycle-method-not-run
    public static void staticTearDown() {}

    @Before
    public void setUp() {}

    @Test
    public void works() {}
}
