// mote-test: path src/main/java/woolfel/SampleTest.java
import org.junit.Test;

public class SampleTest extends SampleTestCase {
    public SampleTest(String name) {
        super(name);
        System.out.println("constructed " + name);
    }

    public void testOne() {
        System.out.println("one");
    }
}

class Annotated {
    @Test
    public void runs() {
        System.out.println("running");
    }

    public void helper() {
        System.out.println("helper");                               // expect: system-out-print
    }
}
