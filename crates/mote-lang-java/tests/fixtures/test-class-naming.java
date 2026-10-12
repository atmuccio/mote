// mote-test: test-file
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.Nested;
import org.junit.jupiter.params.ParameterizedTest;

class InvoiceParsing {                                        // expect: test-class-naming
    @Test
    void readsTotals() {
    }

    @Nested
    class WhenEmpty {
        @Test
        void isZero() {
        }
    }
}

class InvoiceParserTest {
    @Test
    void readsTotals() {
    }
}

class InvoiceParserTests {
    @ParameterizedTest
    void readsTotals() {
    }
}

class InvoiceParserTestCase {
    @Test
    void readsTotals() {
    }
}

class InvoiceParserIT {
    @Test
    void readsTotals() {
    }
}

// Maven's default includes also run `Test*` classes.
class TestInvoiceParser {
    @Test
    void readsTotals() {
    }
}

class ParameterizedChecks {                                   // expect: test-class-naming
    @ParameterizedTest
    void each() {
    }
}

// `IT` is accepted as an ending only, `TestCase` as an ending only.
class ITInvoiceParser {                                       // expect: test-class-naming
    @Test
    void readsTotals() {
    }
}

class InvoiceParserTestCases {                                // expect: test-class-naming
    @Test
    void readsTotals() {
    }
}

abstract class AbstractParserCase {
    @Test
    void shared() {
    }
}

class TestSupport {
    void helper() {
    }
}
