// mote-test: path src/main/java/org/example/support/BaseTestCase.java
import junit.framework.TestCase;

public abstract class BaseTestCase extends TestCase {
    protected void setUp() throws Exception {
        Object provider = new Object() {
            public void destroy() {
            }
        };
    }

    protected void hook() {
    }
}

class Service {
    // expect-next-line: empty-method
    public void start() {
    }
}
