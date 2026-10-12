// mote-test: path src/main/java/com/acme/SelfCheck.java
// mote-test: params max=2
import static org.junit.jupiter.api.Assertions.assertEquals;

import org.junit.jupiter.api.Test;

class SelfCheck {
    @Test
    void checks() {                                                          // expect: too-many-assertions
        assertEquals(1, one());
        assertEquals(1, one());
        assertEquals(1, one());
    }

    static int one() {
        return 1;
    }
}
