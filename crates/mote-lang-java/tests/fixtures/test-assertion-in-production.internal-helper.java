// mote-test: path app/src/main/java/com/acme/report/Exporter.java
package com.acme.report;

import static org.assertj.core.internal.Arrays.assertNotNull;
import static org.assertj.core.util.Preconditions.checkArgument;
import static org.junit.jupiter.api.Assertions.assertEquals;

// Only the public assertion classes count: an internal helper of the library named assert*
// is not a test assertion.
class Exporter {
    void export(Object info, Object rows) {
        assertNotNull(info, rows);
        checkArgument(rows != null, "rows");
        assertEquals(1, 1);                                          // expect: test-assertion-in-production
    }
}
