import java.util.ArrayList;
import java.util.Collection;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.stream.IntStream;
import java.util.stream.Stream;

class ConstructorReferenceCapacity {
    Map<Integer, List<String>> byId = new HashMap<>();

    void wrong(Map<Character, Set<String>> byLetter, Stream<Integer> ids) {
        byId.computeIfAbsent(42, ArrayList::new).add("x");                   // expect: constructor-reference-capacity
        byLetter.computeIfAbsent('a', HashSet::new);                         // expect: constructor-reference-capacity
        IntStream.range(0, 10).mapToObj(StringBuilder::new);                 // expect: constructor-reference-capacity
        ids.map(ArrayList::new);                                             // expect: constructor-reference-capacity
        this.byId.computeIfAbsent(7, java.util.ArrayList::new);              // expect: constructor-reference-capacity
    }

    // Compliant
    void fine(Map<String, List<String>> byName, Map<Integer, List<String>> m,
              Stream<Collection<String>> groups, Stream<String> names, Unknown u) {
        byName.computeIfAbsent("k", k -> new ArrayList<>());
        m.computeIfAbsent(1, k -> new ArrayList<>());
        groups.map(ArrayList::new);                                          // copies each collection
        names.map(StringBuilder::new);                                       // content, not capacity
        Stream.generate(ArrayList::new).limit(3);                            // a supplier
        m.computeIfAbsent(1, Holder::new);
        u.computeIfAbsent(1, ArrayList::new);
    }

    static class Holder extends ArrayList<String> {
        Holder(Integer id) { }
    }

    interface Unknown { Object computeIfAbsent(int k, Object f); }
}
