// mote-test: test-file
package org.example.pool;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotEquals;

import java.util.List;
import org.junit.jupiter.api.Test;

class PoolEntryTest {
    private static final String NAME = "n";

    @Test
    void equalsContract() {
        final PoolEntry a = new PoolEntry(42);
        final PoolEntry b = new PoolEntry(42);
        assertEquals(a, b);
        // The entry under test must be the receiver of equals(): a foreign type on purpose.
        assertNotEquals(a, "42");
        assertNotEquals(b, NAME);
        Wrapper<String> w = new Wrapper<>("x");
        assertNotEquals(w, "x");
    }

    @Test
    void sameTypeStillReported(List<String> names) {
        String first = names.get(0);
        assertEquals(first, "alpha");                               // expect: assert-argument-order
        Object any = names.get(1);
        assertEquals(any, "beta");                                  // expect: assert-argument-order
        CharSequence seq = names.get(2);
        assertEquals(seq, "gamma");                                 // expect: assert-argument-order
        Long count = (long) names.size();
        assertEquals(count, 3L);                                    // expect: assert-argument-order
        var inferred = names.get(3);
        assertEquals(inferred, "delta");                            // expect: assert-argument-order
        PoolEntry e = new PoolEntry(1);
        assertEquals(e.id(), 1);                                    // expect: assert-argument-order
    }

    <T> void typeVariable(T value) {
        assertEquals(value, "t");                                   // expect: assert-argument-order
    }
}
