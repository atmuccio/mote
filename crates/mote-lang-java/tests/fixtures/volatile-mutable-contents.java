import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.CopyOnWriteArrayList;

class VolatileMutableContents {
    private volatile Map<String, Object> sessions = new HashMap<>();         // expect: volatile-mutable-contents
    private volatile int[] counters = new int[8];                            // expect: volatile-mutable-contents
    private volatile List<String> names;                                     // expect: volatile-mutable-contents

    void open(String id, Object s) {
        sessions.put(id, s);
    }

    void hit(int i) {
        this.counters[i]++;
    }

    void reset() {
        names = new ArrayList<>();
    }

    void add(String n) {
        names.add(n);
    }

    // Compliant
    private volatile List<String> snapshot = new ArrayList<>();              // copy on write
    private volatile Map<String, Object> safe = new ConcurrentHashMap<>();
    private volatile List<String> cow = new CopyOnWriteArrayList<>();
    private volatile Object[] table = new Object[4];
    private volatile List<String> locked = new ArrayList<>();
    private volatile List<String> unknown = List.of();
    private volatile int[] built;
    private final Map<String, Object> plain = new HashMap<>();

    VolatileMutableContents() {
        built = new int[2];
        built[0] = 1;                                                        // before the object is shared
    }

    void publish(String n) {
        List<String> copy = new ArrayList<>(snapshot);
        copy.add(n);
        snapshot = copy;
        safe.put(n, n);
        cow.add(n);
        Object[] t = table;
        t[0] = n;
        synchronized (this) {
            locked.add(n);
        }
        plain.put(n, n);
        List<String> local = new ArrayList<>();
        local.add(n);
    }

    synchronized void addLocked(String n) {
        locked.add(n);
    }

    void replaceUnknown(List<String> from) {
        unknown = from;
        unknown.add("x");                                                    // the list's type is unknown
    }
}
