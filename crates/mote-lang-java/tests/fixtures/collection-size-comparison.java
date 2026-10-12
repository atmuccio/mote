import java.util.*;
import java.util.concurrent.ConcurrentHashMap;

class SizeCheck {
    private final List<String> items = new ArrayList<>();
    private Map<String, Integer> counts = new HashMap<>();

    void positives(Set<Integer> set, String s, Deque<String> dq, List raw) {
        if (items.size() == 0) { }                // expect: collection-size-comparison
        if (items.size() != 0) { }                // expect: collection-size-comparison
        if (set.size() > 0) { }                   // expect: collection-size-comparison
        if (0 < set.size()) { }                   // expect: collection-size-comparison
        if (counts.size() < 1) { }                // expect: collection-size-comparison
        if (dq.size() >= 1) { }                   // expect: collection-size-comparison
        if (1 > this.counts.size()) { }           // expect: collection-size-comparison
        if (s.length() == 0) { }                  // expect: collection-size-comparison
        var copy = new ArrayList<>(items);
        boolean empty = copy.size() <= 0;         // expect: collection-size-comparison
        ConcurrentHashMap<String, String> chm = new ConcurrentHashMap<>();
        if (chm.size() == 0) { }                  // expect: collection-size-comparison
        if (counts.keySet().size() == 0) { }      // expect: collection-size-comparison
        if (raw.size() == 0) { }                  // expect: collection-size-comparison
        if (s.trim().length() > 0) { }            // expect: collection-size-comparison
    }

    void negatives(StringBuilder sb, int[] arr, Custom custom) {
        if (items.size() == 1) { }        // not an emptiness check
        if (items.size() > 1) { }
        if (sb.length() == 0) { }         // StringBuilder.isEmpty() needs Java 15+
        if (arr.length == 0) { }          // arrays have no isEmpty()
        if (custom.size() == 0) { }       // unknown type
        if (size() == 0) { }              // unqualified call
        int n = items.size();
        if (n == 0) { }
        if (items.size() == items.size()) { }
    }

    int size() {
        return 0;
    }

    public boolean isEmpty() {
        return items.size() == 0;         // inside isEmpty(): no suggestion
    }
}
