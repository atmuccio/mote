import java.util.concurrent.TimeUnit;
import java.util.function.Consumer;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

class KeystoreLoader {
    private static final Logger log = LoggerFactory.getLogger(KeystoreLoader.class);
    private final Mapper mapper = new Mapper();

    void load(Store store, Store other) {
        if (log.isDebugEnabled()) {
            dumpStore(store);
        }
        if (log.isTraceEnabled()) {
            dumpStore(other);
        }
        if (log.isDebugEnabled() && store != null) {
            dumpStore(store);
        }
        logOnce(store);
        if (log.isDebugEnabled()) {
            logOnce(other);
        }
        // dumpStore(store) in a comment is not a call
        String label = "dumpStore(store)";
        Consumer<Store> sink = this::referenced;
    }

    // Every call is under a debug/trace check: nothing inside is reported.
    private void dumpStore(Store store) {
        log.debug("Store {}", mapper.toJson(store));
        log.debug("Entries {}", store.describeEntries());
        if (store.hasParent()) {
            dumpStore(store.parent());
        }
    }

    // One call site is unguarded.
    private void logOnce(Store store) {
        log.debug("Store {}", mapper.toJson(store));                   // expect: unguarded-log-argument
    }

    // Never called in this file.
    private void neverCalled(Store store) {
        log.debug("Store {}", mapper.toJson(store));                   // expect: unguarded-log-argument
    }

    // Used as a method reference: it may run anywhere.
    private void referenced(Store store) {
        log.debug("Store {}", mapper.toJson(store));                   // expect: unguarded-log-argument
    }

    // Not private: callers in other files are not visible.
    void visibleHelper(Store store) {
        log.debug("Store {}", mapper.toJson(store));                   // expect: unguarded-log-argument
    }

    void callsVisibleHelper(Store store) {
        if (log.isDebugEnabled()) {
            visibleHelper(store);
        }
    }

    // Guarded at the call site by an info check only: not a debug guard.
    private void infoGuarded(Store store) {
        log.debug("Store {}", mapper.toJson(store));                   // expect: unguarded-log-argument
    }

    void callsInfoGuarded(Store store) {
        if (log.isInfoEnabled()) {
            infoGuarded(store);
        }
    }

    // Called when debug logging is off, and from an else branch.
    private void negated(Store store) {
        log.debug("Store {}", mapper.toJson(store));                   // expect: unguarded-log-argument
    }

    private void inElse(Store store) {
        log.debug("Store {}", mapper.toJson(store));                   // expect: unguarded-log-argument
    }

    void callsNegated(Store store) {
        if (!log.isDebugEnabled()) {
            negated(store);
        }
        if (log.isDebugEnabled()) {
            dumpStore(store);
        } else {
            inElse(store);
        }
        String shown = log.isTraceEnabled() ? traced(store) : "-";
    }

    // Only called as the value of a ternary guarded by a trace check.
    private String traced(Store store) {
        log.trace("Store {}", mapper.toJson(store));
        return "x";
    }

    enum Kind { PRIMARY, BACKUP }

    enum Described {
        FIRST;

        @Override
        public String toString() {
            return name().toLowerCase() + " (described)";
        }
    }

    void conversions(boolean enabled, int count, Kind kind, TimeUnit unit, Described described,
                     Status status, Object any) {
        log.debug("Enabled {}", String.valueOf(enabled));
        log.debug("Count {}", String.valueOf(count));
        log.debug("Kind {}", kind.toString());
        log.debug("Unit {}", unit.toString());
        log.debug("Current {}", current().toString());
        log.debug("Described {}", described.toString());               // expect: unguarded-log-argument
        log.debug("Status {}", status.toString());                     // expect: unguarded-log-argument
        log.debug("Any {}", String.valueOf(any));                      // expect: unguarded-log-argument
        log.debug("Twice {}", String.valueOf(twice(count)));           // expect: unguarded-log-argument
        log.debug("Kind {}", kinds().toString());                      // expect: unguarded-log-argument
    }

    Kind current() {
        return Kind.PRIMARY;
    }

    int twice(int n) {
        return n * 2;
    }

    Kind[] kinds() {
        return Kind.values();
    }
}
