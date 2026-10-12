// mote-test: test-file
// A `@Test` annotation from an unknown library is not a test framework's.
import com.example.annotations.Test;
import org.testng.annotations.Test2;

class Checks {
    @Test
    void run() {
    }
}
