// mote-test: test-file
import com.example.testing.UnitTest;
import org.junit.jupiter.api.Test;
import org.mockito.Mock;

// A project annotation may be meta-annotated with the Mockito extension: not reported.
@UnitTest
class CustomAnnotatedTest {
    @Mock
    Runnable task;

    @Test
    void runs() {
        task.run();
    }
}

abstract class AbstractBaseTest {
    @Mock
    Runnable shared;

    @Test
    void base() {}
}

class SubclassTest extends AbstractBaseTest {
    @Mock
    Runnable own;

    @Test
    void sub() {}
}

class HelperWithoutTests {
    @Mock
    Runnable r;
}
