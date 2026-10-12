// mote-test: test-file
import java.util.List;

// Test doubles return null on purpose.
class StubRepository implements Repository {
    public List<String> names() {
        return null;
    }
}
