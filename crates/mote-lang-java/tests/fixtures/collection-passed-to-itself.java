import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;

class Merge {
    private final List<String> merged = new ArrayList<>();

    void wrong(Set<String> allowed, Map<String, Integer> totals, List<String> other) {
        allowed.retainAll(allowed);                 // expect: collection-passed-to-itself
        merged.addAll(merged);                      // expect: collection-passed-to-itself
        boolean all = other.containsAll(other);     // expect: collection-passed-to-itself
        totals.putAll(totals);                      // expect: collection-passed-to-itself
        this.merged.removeAll(this.merged);         // expect: collection-passed-to-itself
    }

    void fine(Set<String> allowed, Set<String> granted, Holder h, Custom custom, List<String> other) {
        allowed.retainAll(granted);
        merged.addAll(other);
        h.list().addAll(h.list());
        custom.addAll(custom);
        merged.addAll(this.merged.subList(0, 1));
        List<String> copy = new ArrayList<>(merged);
        copy.addAll(merged);
        this.merged.addAll(merged);                 // different spelling: not matched textually
    }

    interface Holder { List<String> list(); }
    interface Custom { void addAll(Custom c); }
}
