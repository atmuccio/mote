// mote-test: test-file
import static org.junit.Assert.assertFalse;

class MoneyTest {
    void equalsContract() {
        Money m = new Money(5);
        assertFalse(m.equals(null));        // verifies the equals contract
    }
}
