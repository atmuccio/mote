import java.util.List;
import java.util.Map;
import java.util.Set;

// The same names are declared with different types in different methods: the syntactic
// pre-check cannot tell them apart, so the rule stays silent rather than guess.
class Shadowing {
    void first(Set<Long> ids, Map<String, Integer> index) {
        ids.contains("7");
        index.get(3L);
    }

    void second(Set<String> ids, Map<Long, Integer> index) {
        ids.contains("7");
        index.get(3L);
    }

    void chains(Holder h, List<Long> values, long v) {
        h.ids().contains("x");                                      // call result: not checked
        values.contains(v);                                         // long boxes to Long
        values.contains(h.count());                                 // method-call argument: not checked
    }
}
