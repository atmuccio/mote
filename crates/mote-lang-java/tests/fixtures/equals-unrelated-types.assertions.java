// mote-test: test-file
import static org.assertj.core.api.Assertions.assertThat;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotEquals;

import java.util.UUID;
import java.util.concurrent.TimeUnit;
import org.junit.jupiter.api.Assertions;

class OrderTest {
    enum Status { ACTIVE, CLOSED }

    void positives(int count, String name, Status status, UUID id, StringBuilder sb, TimeUnit unit) {
        assertEquals("3", count);                                   // expect: equals-unrelated-types
        assertEquals(status, "ACTIVE");                             // expect: equals-unrelated-types
        assertNotEquals(id, name);                                  // expect: equals-unrelated-types
        Assertions.assertEquals(name, sb);                          // expect: equals-unrelated-types
        assertThat(name).isEqualTo(count);                          // expect: equals-unrelated-types
        assertThat(status).isNotEqualTo("CLOSED");                  // expect: equals-unrelated-types
        assertEquals(unit, "SECONDS");                              // expect: equals-unrelated-types
        assertEquals(TimeUnit.SECONDS, "SECONDS");                  // type not visible in the syntax: skipped
    }

    void negatives(int count, long total, Long boxed, Integer n, String name, Object any, CharSequence cs) {
        assertEquals(3, count);
        assertEquals(1L, count);                                    // widened by assertEquals(long, long)
        assertEquals(boxed, n);                                     // numeric pair: overloads decide
        assertThat(total).isEqualTo(3);                             // isEqualTo(long)
        assertEquals("x", name);
        assertEquals(name, any);                                    // Object may be a String
        assertEquals(name, cs);                                     // CharSequence may be a String
        assertEquals("expected", name, "message");                  // three arguments: not checked
        assertEquals(null, name);
        assertEquals('a', count);                                   // char and int: primitive overloads
        custom.assertEquals("3", count);                            // not a test library
    }

    Custom custom = new Custom();
    static class Custom { void assertEquals(Object a, Object b) {} }
}
