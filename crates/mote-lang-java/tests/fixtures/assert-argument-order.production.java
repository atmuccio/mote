import static org.junit.jupiter.api.Assertions.assertEquals;

// Not a test file: the rule stays silent.
class Validator {
    void check(Invoice invoice) {
        assertEquals(invoice.total(), 42);
    }
}
