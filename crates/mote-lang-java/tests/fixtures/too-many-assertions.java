// mote-test: test-file
// mote-test: params max=3
import static org.assertj.core.api.Assertions.assertThat;
import static org.junit.jupiter.api.Assertions.assertAll;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

import org.junit.jupiter.api.Test;

class ParserTest {
    @Test
    void parsesEverything() {                                                // expect: too-many-assertions
        assertEquals(1, parse("1"));
        assertEquals(2, parse("2"));
        assertTrue(parse("3") > 2);
        assertThat(parse("4")).isPositive().isEqualTo(4);
    }

    @Test
    void grouped() {                                                         // expect: too-many-assertions
        assertAll(
                () -> assertEquals(1, parse("1")),
                () -> assertEquals(2, parse("2")),
                () -> assertEquals(3, parse("3")),
                () -> assertEquals(4, parse("4")));
    }

    public void testLegacyStyle() {                                          // expect: too-many-assertions
        assert parse("1") == 1;
        assert parse("2") == 2;
        assertEquals(3, parse("3"));
        assertEquals(4, parse("4"));
    }

    // Compliant
    @Test
    void fluentChainsCountOnce() {
        assertThat(parse("1")).isPositive().isEqualTo(1).isLessThan(2).isNotZero();
        assertThat(parse("2")).isEqualTo(2);
        assertThat(parse("3")).isEqualTo(3);
    }

    @Test
    void loopCountsOnce() {
        for (int i = 0; i < 10; i++) {
            assertEquals(i, parse(String.valueOf(i)));
        }
    }

    void helperNotATest() {
        assertEquals(1, parse("1"));
        assertEquals(2, parse("2"));
        assertEquals(3, parse("3"));
        assertEquals(4, parse("4"));
    }

    @Test
    void comments() {
        // assert assert assert assert assert
        assertEquals(1, parse("1"));
    }

    @Test
    void anonymousHelpers() {
        Runnable r = new Runnable() {
            public void run() {
                assertEquals(1, parse("1"));
                assertEquals(2, parse("2"));
                assertEquals(3, parse("3"));
                assertEquals(4, parse("4"));
            }
        };
    }

    static int parse(String s) {
        return 0;
    }
}
