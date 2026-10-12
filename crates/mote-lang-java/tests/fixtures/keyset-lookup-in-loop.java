import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;

class Settings {
    private final Map<String, String> settings = new HashMap<>();
    private Multimap<String, String> multi;

    void positives(TreeMap<String, Integer> counts, Map<String, List<String>> groups) {
        for (String name : settings.keySet()) {                    // expect: keyset-lookup-in-loop
            apply(name, settings.get(name));
        }
        for (String name : this.settings.keySet()) {               // expect: keyset-lookup-in-loop
            if (name.startsWith("x")) {
                apply(name, settings.get(name));
            }
        }
        for (String key : counts.keySet()) {                       // expect: keyset-lookup-in-loop
            total += counts.get(key);
        }
        for (var key : groups.keySet()) {                          // expect: keyset-lookup-in-loop
            groups.get(key).clear();
        }
    }

    void negatives(Map<String, String> other, Map<String, String> copy, Object untyped) {
        // Only the keys are used.
        for (String name : settings.keySet()) {
            apply(name, null);
        }
        // Another map.
        for (String name : settings.keySet()) {
            apply(name, copy.get(name));
        }
        // Another key.
        for (String name : settings.keySet()) {
            apply(name, settings.get(name.toUpperCase()));
        }
        // Inside a lambda.
        for (String name : settings.keySet()) {
            Runnable r = () -> apply(name, settings.get(name));
        }
        // Not a java.util.Map.
        for (String name : multi.keySet()) {
            apply(name, String.valueOf(multi.get(name)));
        }
        // Already iterating entries.
        for (Map.Entry<String, String> e : settings.entrySet()) {
            apply(e.getKey(), e.getValue());
        }
        // Loop variable reassigned.
        for (String name : settings.keySet()) {
            name = name.trim();
            apply(name, settings.get(name));
        }
    }

    interface Multimap<K, V> {
        java.util.Set<K> keySet();
        java.util.Collection<V> get(K key);
    }

    private int total;

    void apply(String k, String v) {}
}
