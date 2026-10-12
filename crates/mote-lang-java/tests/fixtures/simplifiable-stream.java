import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Optional;
import java.util.Set;
import java.util.stream.Collectors;
import java.util.stream.Stream;

class Pipelines {
    boolean positives(List<Order> orders, List<String> names, Stream<String> words) {
        boolean a = orders.stream().filter(o -> o.isLate()).findFirst().isPresent();   // expect: simplifiable-stream
        boolean b = names.stream().filter(String::isEmpty).findAny().isEmpty();        // expect: simplifiable-stream
        boolean c = names.stream().map(String::trim).filter(n -> n.length() > 3).count() > 0; // expect: simplifiable-stream
        boolean d = words.filter(w -> w.startsWith("x")).count() == 0;                 // expect: simplifiable-stream
        boolean e = 0 != names.stream().filter(n -> n.isBlank()).count();              // expect: simplifiable-stream
        long f = names.stream().collect(Collectors.counting());                        // expect: simplifiable-stream
        names.stream().forEach(n -> System.out.println(n));                            // expect: simplifiable-stream
        List<String> upper = new ArrayList<>();
        names.stream().map(String::toUpperCase).forEach(upper::add);                   // expect: simplifiable-stream
        Set<String> trimmed = new HashSet<>();
        names.stream().map(String::trim).forEach(t -> trimmed.add(t));                 // expect: simplifiable-stream
        return a || b || c || d || e || f > 0;
    }

    boolean negatives(List<String> names, Store store, Optional<String> maybe) {
        boolean a = store.items().filter(i -> i.ok()).findFirst().isPresent();   // unknown type: not a proven stream
        boolean b = maybe.filter(String::isEmpty).isPresent();                   // Optional.filter
        boolean c = names.stream().filter(n -> n.isEmpty()).count() > 2;          // a real count
        boolean d = names.stream().count() > 0;                                   // no filter
        names.parallelStream().forEach(n -> System.out.println(n));               // deliberately parallel
        names.forEach(n -> System.out.println(n));
        List<String> upper = new ArrayList<>();
        upper.add("first");
        names.stream().map(String::toUpperCase).forEach(upper::add);              // not fresh
        ArrayList<String> concrete = new ArrayList<>();
        names.stream().map(String::trim).forEach(concrete::add);                  // concrete declared type
        List<String> other = new ArrayList<>();
        names.stream().map(String::trim).forEach(n -> other.add(n + "!"));        // adds something else
        boolean e = names.stream().anyMatch(String::isEmpty);
        long f = names.stream().collect(Collectors.groupingBy(String::length, Collectors.counting())).size();
        return a || b || c || d || e || f > 0;
    }

    interface Store {
        Items items();
    }
}
