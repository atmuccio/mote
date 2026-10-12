// mote-test: test-file
import org.junit.jupiter.api.Test;

class LocalHelperTest {
    @Test
    void usesProjectHelper(int total) {
        Assertions.assertThat(total > 0);
        Checks.assertThat(total);
    }

    static final class Assertions {
        static void assertThat(boolean condition) {
            if (!condition) {
                throw new AssertionError();
            }
        }
    }

    static final class Checks {
        static void assertThat(int value) {
            if (value < 0) {
                throw new AssertionError();
            }
        }
    }
}
