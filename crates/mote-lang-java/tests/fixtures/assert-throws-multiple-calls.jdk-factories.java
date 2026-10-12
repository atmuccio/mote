// mote-test: test-file
import static com.acme.testing.Samples.sampleOrder;
import static java.nio.file.Files.readString;
import static java.util.Arrays.asList;
import static java.util.Collections.singletonList;
import static org.junit.jupiter.api.Assertions.assertThrows;

import java.nio.file.Path;
import java.util.Collections;
import java.util.Hashtable;
import javax.naming.InitialContext;
import javax.swing.RowSorter.SortKey;
import javax.swing.SortOrder;
import org.junit.jupiter.api.Test;

class GridSorterTest {
    private final GridSorter sorter = new GridSorter();

    @Test
    void jdkFactoriesAndSwingValues() {
        // Statically imported JDK factories and Swing value constructors cannot fail.
        assertThrows(IllegalArgumentException.class,
                () -> sorter.setSortKeys(asList(new SortKey(0, SortOrder.ASCENDING), new SortKey(1, SortOrder.DESCENDING))));
        assertThrows(IllegalArgumentException.class,
                () -> sorter.setSortKeys(Collections.singletonList(new SortKey(7, SortOrder.ASCENDING))));
        assertThrows(IllegalArgumentException.class,
                () -> sorter.setSortKeys(singletonList(new SortKey(3, SortOrder.UNSORTED))));
    }

    @Test
    void setupThatCanFail(Path file, Hashtable<String, String> env) {
        assertThrows(IllegalStateException.class, () -> sorter.load(sampleOrder(3)));             // expect: assert-throws-multiple-calls
        assertThrows(IllegalStateException.class, () -> sorter.parse(readString(file)));          // expect: assert-throws-multiple-calls
        assertThrows(IllegalStateException.class, () -> sorter.bind(new InitialContext(env)));    // expect: assert-throws-multiple-calls
    }
}

class ShadowingTest {
    private final GridSorter sorter = new GridSorter();

    // A method of the class wins over the static import of the same name.
    static java.util.List<String> asList(String... names) {
        return java.util.List.of(names);
    }

    @Test
    void ownHelper() {
        assertThrows(IllegalArgumentException.class, () -> sorter.names(asList("a", "b")));    // expect: assert-throws-multiple-calls
    }
}
