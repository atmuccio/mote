// mote-test: test-file
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;

import org.junit.jupiter.api.Disabled;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Nested;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.Timeout;
import org.junit.jupiter.params.ParameterizedTest;
import org.junit.jupiter.params.provider.ValueSource;

class NumbersTest {
    @Test
    void parsesOne() {                                                      // expect: test-could-be-parameterized
        assertEquals(1, Numbers.parse("one"));
    }

    @Test
    @DisplayName("two")
    void parsesTwo() {
        // a comment does not matter
        assertEquals(2, Numbers.parse("two"));
    }

    @Test
    void parsesThree() {
        assertEquals(3, Numbers.parse("three"));
    }

    @Test
    void rejectsBlank() {
        assertThrows(IllegalArgumentException.class, () -> Numbers.parse(" "));
    }

    @Nested
    class Formatting {
        @Test
        void formatsSmall() throws Exception {                              // expect: test-could-be-parameterized
            Numbers n = new Numbers();
            assertEquals("1", n.format(1, 'x', true));
        }

        @Test
        void formatsLarge() throws Exception {
            Numbers n = new Numbers();
            assertEquals("1000", n.format(1000, 'y', false));
        }

        @Test
        void formatsZero() throws Exception {
            Numbers n = new Numbers();
            assertEquals("0", n.format(0, 'x', true));
        }
    }

    static class Numbers {
        static int parse(String s) { return 0; }
        String format(long v, char c, boolean b) { return ""; }
    }
}

class CompliantTest {
    // Only two alike.
    @Test void a() { assertEquals(1, Numbers2.count("one")); }
    @Test void b() { assertEquals(2, Numbers2.count("two")); }

    // Literal kinds differ: 1, 1L and 1.0 may pick different overloads.
    @Test void w1() { assertEquals(1, Numbers2.write(1)); }
    @Test void w2() { assertEquals(1, Numbers2.write(1L)); }
    @Test void w3() { assertEquals(1, Numbers2.write(1.0)); }

    // Annotations differ.
    @Test void t1() { assertEquals(1, Numbers2.twice(1)); }
    @Test @Disabled("flaky") void t2() { assertEquals(4, Numbers2.twice(2)); }
    @Test @Timeout(5) void t3() { assertEquals(6, Numbers2.twice(3)); }

    // Identical copies (another rule).
    @Test void c1() { assertEquals(0, Numbers2.negate(0)); }
    @Test void c2() { assertEquals(0, Numbers2.negate(0)); }
    @Test void c3() { assertEquals(0, Numbers2.negate(0)); }

    // Different calls.
    @Test void d1() { assertEquals(1, Numbers2.parse("a")); }
    @Test void d2() { assertEquals(1, Numbers2.twice(1)); }
    @Test void d3() { assertEquals(1, Numbers2.write(1)); }

    // Already parameterized.
    @ParameterizedTest @ValueSource(ints = {1, 2}) void p1(int v) { assertEquals(v, Numbers2.twice(v)); }

    static class Numbers2 {
        static int parse(String s) { return 0; }
        static int count(String s) { return 0; }
        static int negate(int v) { return -v; }
        static int twice(int v) { return 2 * v; }
        static int write(Object o) { return 1; }
    }
}
