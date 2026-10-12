// mote-test: test-file
import static org.junit.Assert.assertTrue;

class EqualsContractTest {
    void reflexive() {
        Money m = new Money(5);
        assertTrue(m.equals(m));                // reflexivity check: fine in tests
        int a = 3;
        assertTrue(a == a);                     // expect: identical-operands
    }
}
