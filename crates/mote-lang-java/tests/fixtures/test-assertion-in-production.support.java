// mote-test: path src/main/java/com/acme/testing/Fixtures.java
package com.acme.testing;

import static org.junit.Assert.assertNotNull;

// Test-support code shipped in a main source set: not reported.
class Fixtures {
    void check(Object o) {
        assertNotNull(o);
    }
}
