// mote-test: test-file
import org.testng.annotations.Test;

// Helpers of an unknown base class may assert: unqualified calls are trusted.
public class RouterTest extends AbstractRouterTest {
    @Test
    public void routes() {
        runScenario("simple");
    }

    @Test(expectedExceptions = IllegalStateException.class)
    public void failsOnBadInput() {
        router.route(null);
    }

    @Test
    public void onlyQualifiedCalls() {                              // expect: test-without-assertion
        router.route("x");
    }
}

class NotJUnit {
    @Test2
    void t() { run(); }

    @com.acme.Test
    void u() { run(); }
}
