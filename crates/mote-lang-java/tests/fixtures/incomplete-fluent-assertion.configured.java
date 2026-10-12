// mote-test: test-file
import static org.assertj.core.api.Assertions.assertThat;

import java.util.Comparator;
import java.util.List;
import java.util.Optional;
import org.junit.jupiter.api.Test;

class ConfiguredTest {
    @Test
    void unfinished(Item item, List<Item> items, Comparator<Item> byName) {
        assertThat(item).usingRecursiveComparison();                        // expect: incomplete-fluent-assertion
        assertThat(item).usingComparator(byName);                           // expect: incomplete-fluent-assertion
        assertThat(items).extracting("name");                               // expect: incomplete-fluent-assertion
        assertThat(items).as("names").extracting(Item::name);               // expect: incomplete-fluent-assertion
        assertThat(items).allSatisfy(e -> assertThat(e.name()));            // expect: incomplete-fluent-assertion
        assertThat(item).satisfies(e -> assertThat(e.id()).as("id"));       // expect: incomplete-fluent-assertion
        items.forEach(e -> assertThat(e.id() > 0));                         // expect: incomplete-fluent-assertion
    }

    @Test
    void finished(Item item, List<Item> items, Comparator<Item> byName, Optional<Item> maybe) {
        assertThat(item).usingRecursiveComparison().isEqualTo(item);
        assertThat(item).usingRecursiveComparison().ignoringFields("id");   // ends in another call
        assertThat(item).usingComparator(byName).isEqualTo(item);
        assertThat(items).extracting("name").contains("a");
        assertThat(items).allSatisfy(e -> assertThat(e.name()).isNotEmpty());
        assertThat(item).satisfies(e -> {
            assertThat(e.id()).isPositive();
        });
        java.util.function.Supplier<Object> s = () -> assertThat(item);    // value returned to the caller
        maybe.map(e -> assertThat(e.name()));                               // value used by map
        items.stream().map(Item::name).extracting();                        // not a fluent assertion
    }

    interface Item {
        String name();
        int id();
    }
}
