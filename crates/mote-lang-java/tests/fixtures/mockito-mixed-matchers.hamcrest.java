// mote-test: test-file
import static org.hamcrest.MatcherAssert.assertThat;
import static org.hamcrest.Matchers.*;
import static org.mockito.ArgumentMatchers.*;

import java.util.List;

// Hamcrest's `contains` and `startsWith` share names with Mockito matchers: not reported.
class ListTest {
    void check(List<String> list, String name) {
        assertThat(list, contains("a", "b"));
        assertThat(name, startsWith("x"));
    }
}
