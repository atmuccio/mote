// mote-test: test-file
import static org.junit.Assert.assertEquals;

import org.junit.Assert;
import org.junit.jupiter.api.Assertions;

class BooleanAssertTest {
    void positives(Cache cache, Connection conn) {
        assertEquals(true, cache.isEmpty());                        // expect: assert-boolean-literal
        assertEquals(false, conn.isOpen());                         // expect: assert-boolean-literal
        assertEquals("must be closed", false, conn.isOpen());       // expect: assert-boolean-literal
        Assertions.assertEquals(true, cache.isEmpty(), "empty");    // expect: assert-boolean-literal
        Assert.assertEquals(cache.isEmpty(), true);                 // expect: assert-boolean-literal
        assertEquals(                                               // expect: assert-boolean-literal
            (true), cache.isEmpty());
    }

    void negatives(Cache cache) {
        assertTrue(cache.isEmpty());
        assertEquals(1, cache.size());
        assertEquals(Boolean.TRUE, cache.flag());
        assertEquals("true", cache.text());
        assertNotEquals(true, cache.isEmpty());
        assertEquals(1.0, cache.ratio(), 0.01);
        assertEquals(true);                                         // odd arity
    }
}

class OwnAssertEquals {
    void assertEquals(boolean expected, Object actual) { }
    void m() {
        assertEquals(true, value());                                // project helper
    }
}
