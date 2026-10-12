import java.util.HashMap;
import java.util.Map;

class Registry {
    private volatile Registry instance;
    private volatile Map<String, String> table;
    private Registry other;
    private final Object lock = new Object();
    int size;

    Registry get() {
        if (instance == null) {
            synchronized (this) {
                if (instance == null) {
                    instance = new Registry();                  // expect: double-checked-publish-early
                    instance.load();
                }
            }
        }
        return instance;
    }

    Map<String, String> table() {
        if (this.table == null) {
            synchronized (lock) {
                if (table == null) {
                    this.table = new HashMap<>();               // expect: double-checked-publish-early
                    log("created");
                    table.put("a", "b");
                }
            }
        }
        return table;
    }

    Registry sized() {
        if (other == null) {
            synchronized (lock) {
                if (other == null) {
                    other = new Registry();                     // expect: double-checked-publish-early
                    other.size = 3;
                }
            }
        }
        return other;
    }

    // Negatives.
    Registry buildFirst() {
        if (instance == null) {
            synchronized (this) {
                if (instance == null) {
                    Registry r = new Registry();
                    r.load();
                    instance = r;
                }
            }
        }
        return instance;
    }

    Registry noSetUp() {
        if (instance == null) {
            synchronized (this) {
                if (instance == null) {
                    instance = new Registry();
                    log("created " + instance);
                }
            }
        }
        return instance;
    }

    void notDoubleChecked() {
        synchronized (this) {
            if (instance == null) {
                instance = new Registry();
                instance.load();
            }
        }
    }

    void local() {
        Registry r = null;
        if (r == null) {
            synchronized (this) {
                if (r == null) {
                    r = new Registry();
                    r.load();
                }
            }
        }
    }

    void load() {}
    void log(String s) {}
}
