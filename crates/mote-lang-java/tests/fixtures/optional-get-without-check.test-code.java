// mote-test: test-file
import java.util.Optional;

class OptionalsTest {
    void lookup() {
        Optional<String> found = find();
        assertEquals("x", found.get());         // a failing get() just fails the test
    }

    Optional<String> find() { return Optional.empty(); }
    void assertEquals(Object a, Object b) {}
}
