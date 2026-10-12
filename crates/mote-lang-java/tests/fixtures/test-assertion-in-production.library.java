// mote-test: path checker-core/src/main/java/org/assertj/core/internal/Grids.java
package org.assertj.core.internal;

import static org.assertj.core.internal.Arrays.assertNotNull;
import static org.junit.Assert.assertTrue;

// The assertion library's own sources: its internal helpers are not test assertions, and its
// own package defines the assertion API.
public class Grids {
    void assertEmpty(Object info, Object grid) {
        assertNotNull(info, grid);
        assertTrue(grid != null);
    }
}
