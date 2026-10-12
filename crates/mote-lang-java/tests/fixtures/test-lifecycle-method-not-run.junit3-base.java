// mote-test: test-file
import org.junit.After;
import org.junit.Before;
import org.junit.Test;

// The base class lives in another file and may be a JUnit 3 TestCase: its mechanism calls
// the protected setUp/tearDown overrides itself.
public class QueueUsageTest extends BrokerTestSupport {
    @Before
    protected void setUp() throws Exception {
        super.setUp();
    }

    @After
    protected void tearDown() throws Exception {
        super.tearDown();
    }

    // Other names are not JUnit 3 hooks.
    @Before                                                        // expect: test-lifecycle-method-not-run
    protected void prepare() {
    }

    @Test
    public void usage() { }
}

// No superclass: a protected JUnit 4 hook is never run.
class StandaloneTest {
    @Before                                                        // expect: test-lifecycle-method-not-run
    protected void setUp() { }

    @Test
    public void works() { }
}
