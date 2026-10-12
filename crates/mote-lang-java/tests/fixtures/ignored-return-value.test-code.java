// mote-test: test-file
import static org.junit.jupiter.api.Assertions.assertThrows;

import org.junit.jupiter.api.Test;

class TrimTest {
    @Test
    void throwsOnBadIndex() {
        String s = "abc";
        assertThrows(StringIndexOutOfBoundsException.class, () -> {
            s.substring(9);                                         // provokes the exception on purpose
        });
        s.toUpperCase();                                            // expect: ignored-return-value
    }
}
