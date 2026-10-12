// mote-test: test-file
import org.junit.jupiter.api.Test;

class AddressesTest {
    @Test
    void rejectsNull() {
        try {
            new Addresses().city(null);
        } catch (NullPointerException e) {
            return;
        }
        throw new AssertionError("expected an exception");
    }
}
