// mote-test: test-file
import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertSame;

import java.util.List;
import org.junit.jupiter.api.Assertions;
import org.junit.jupiter.api.Test;

class InvoiceTest {
    private static final String CURRENCY = "EUR";
    private static final int[] EXPECTED_ROWS = {1, 2};
    private final String label = "x";

    enum Status { OPEN, SHIPPED }

    @Test
    void positives(Invoice invoice, List<String> lines) {
        assertEquals(invoice.total(), 42);                          // expect: assert-argument-order
        assertEquals(lines.size(), 3, "three lines");               // expect: assert-argument-order
        assertEquals(invoice.currency(), CURRENCY);                 // expect: assert-argument-order
        assertEquals(invoice.status(), Status.SHIPPED);             // expect: assert-argument-order
        assertNotEquals(invoice.name(), "draft");                   // expect: assert-argument-order
        assertSame(invoice.type(), String.class);                   // expect: assert-argument-order
        Assertions.assertEquals(invoice.rate(), 0.25, 0.001);       // expect: assert-argument-order
        assertArrayEquals(invoice.rows(), EXPECTED_ROWS);           // expect: assert-argument-order
        String actual = invoice.name();
        assertEquals(actual, "final");                              // expect: assert-argument-order
        assertEquals(invoice.delta(), -1);                          // expect: assert-argument-order
    }

    @Test
    void negatives(Invoice invoice, String expectedName) {
        assertEquals(42, invoice.total());
        assertEquals("EUR", invoice.currency());
        assertEquals(CURRENCY, invoice.currency());
        assertEquals(Status.SHIPPED, invoice.status());
        assertEquals(1, 1);                                         // both constant
        assertEquals(CURRENCY, "EUR");                              // both constant
        assertEquals(invoice.done(), true);                         // boolean literal: another rule
        assertEquals(invoice.parent(), null);                       // null: assertNull fits better
        assertEquals(expectedName, "final");                        // first argument named expected
        assertEquals(invoice.name(), label);                        // instance field is not a constant
        assertEquals(invoice.a(), invoice.b());
    }
}
