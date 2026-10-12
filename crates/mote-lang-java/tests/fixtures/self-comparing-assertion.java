// mote-test: test-file
import static org.assertj.core.api.Assertions.assertThat;
import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertSame;

import org.junit.jupiter.api.Test;

class InvoiceTest {
    enum Kind { A, B }

    @Test
    void wrong(Invoice invoice, int[] expected, Kind kind) {
        long expectedTotal = 42L;
        String name = "x";
        assertEquals(expectedTotal, expectedTotal);                     // expect: self-comparing-assertion
        assertEquals("names differ", name, name);                       // expect: self-comparing-assertion
        assertArrayEquals(expected, expected);                          // expect: self-comparing-assertion
        assertSame(invoice, invoice);                                   // expect: self-comparing-assertion
        assertThat(invoice).isEqualTo(invoice);                         // expect: self-comparing-assertion
        assertThat(invoice.id).isSameAs(invoice.id);                    // expect: self-comparing-assertion
        assertEquals(expected.length, expected.length);                 // expect: self-comparing-assertion
        assertNotEquals(kind, kind);                                    // expect: self-comparing-assertion
    }

    @Test
    void fine(Invoice invoice, Invoice other, Service service) {
        assertEquals(42L, invoice.total());
        assertEquals(invoice, invoice);             // reflexivity of a custom equals
        assertEquals(service.next(), service.next());
        assertEquals(1, 1);
        assertEquals("a", "a");
        assertSame(invoice, other);
        assertThat(invoice).isEqualTo(other);
        assertThat(invoice.total()).isEqualTo(invoice.total());
        assertEquals(0.5, invoice.ratio, 0.5);
    }

    static class Invoice {
        Object id;
        double ratio;
        long total() { return 1; }
    }

    interface Service { int next(); }
}
