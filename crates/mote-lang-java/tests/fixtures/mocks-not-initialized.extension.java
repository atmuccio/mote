// mote-test: test-file
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.extension.ExtendWith;
import org.mockito.Mock;
import org.mockito.junit.jupiter.MockitoExtension;

@ExtendWith(MockitoExtension.class)
class WithExtensionTest {
    @Mock
    Runnable task;

    @Test
    void runs() {
        task.run();
    }
}
