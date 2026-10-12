import java.io.Closeable;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

class MapCallbacks {
    private final Map<String, List<String>> index = new HashMap<>();
    private final ConcurrentHashMap<String, StringBuilder> buffers = new ConcurrentHashMap<>();
    private final Map<String, Closeable> open = new ConcurrentHashMap<>();

    void positives(String key, String item) {
        index.computeIfAbsent(key, k -> null);                      // expect: map-callback-returns-null
        index.computeIfAbsent(key, k -> {                           // expect: map-callback-returns-null
            log(k);
            return null;
        });
        index.compute(key, (k, list) -> {                           // expect: map-callback-returns-null
            list.add(item);
            return null;
        });
        buffers.computeIfPresent(key, (k, sb) -> {                  // expect: map-callback-returns-null
            sb.append(item);
            return null;
        });
        index.merge(key, new ArrayList<>(), (old, fresh) -> {       // expect: map-callback-returns-null
            old.addAll(fresh);
            return null;
        });
    }

    void negatives(String key, String item, boolean f) {
        index.computeIfAbsent(key, k -> new ArrayList<>()).add(item);
        index.computeIfAbsent(key, k -> f ? new ArrayList<>() : null);
        index.computeIfAbsent(key, k -> {
            if (f) {
                return null;
            }
            return new ArrayList<>();
        });
        index.compute(key, (k, list) -> {
            if (list == null) {
                list = new ArrayList<>();
            }
            list.add(item);
            return list;
        });
        index.computeIfPresent(key, (k, list) -> null);            // an explicit remove
        open.computeIfPresent(key, (k, c) -> {
            close(c);
            return null;
        });
        index.compute(key, (k, list) -> {
            Runnable r = () -> list.add(item);
            return null;
        });
        index.computeIfAbsent(key, this::create);
        other.computeIfAbsent(key, k -> null);                      // not a java.util.Map
    }

    private final Registry other = new Registry();

    static class Registry {
        Object computeIfAbsent(String k, java.util.function.Function<String, Object> f) {
            return null;
        }
    }

    List<String> create(String k) {
        return new ArrayList<>();
    }

    void close(Closeable c) {}

    void log(String s) {}
}
