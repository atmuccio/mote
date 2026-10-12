// mote-test: test-file
// mote-test: params prefixes=Spec,Check
// Other prefixes replace the default `Test`.
import org.junit.Test;

class SpecInvoiceParser {
    @Test
    public void readsTotals() {
    }
}

class CheckInvoiceParser {
    @Test
    public void readsTotals() {
    }
}

class TestInvoiceParser {                                     // expect: test-class-naming
    @Test
    public void readsTotals() {
    }
}

class InvoiceParserTest {
    @Test
    public void readsTotals() {
    }
}

class InvoiceChecks {                                         // expect: test-class-naming
    @Test
    public void readsTotals() {
    }
}
