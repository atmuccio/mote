// mote-test: test-file
// mote-test: params suffixes=Test
import org.testng.annotations.*;

@Test
class ServiceChecks {                                         // expect: test-class-naming
    void run() {
    }
}

class ServiceTests {                                          // expect: test-class-naming
    @Test
    void run() {
    }
}

class ServiceTest {
    @Test
    void run() {
    }
}
