import java.util.List;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

class Loader {
    private static final Logger log = LoggerFactory.getLogger(Loader.class);
    private static final String PREFIX = "loader: ";
    private final Logger instanceLogger = LoggerFactory.getLogger(getClass());

    void positives(List<String> rows, String table, int count) {
        log.debug("Loaded " + rows.size() + " rows for " + table);   // expect: log-message-concatenation
        log.trace("table=" + table);                                // expect: log-message-concatenation
        log.debug(String.format("%d rows", count));                 // expect: log-message-concatenation
        this.instanceLogger.debug("x" + count, new Exception());    // expect: log-message-concatenation
        log.debug(PREFIX + table);                                  // expect: log-message-concatenation
        log.debug("rows: %s".formatted(rows));                      // expect: log-message-concatenation
    }

    void negatives(List<String> rows, String table, Other other) {
        log.debug("Loaded {} rows for {}", rows.size(), table);
        log.info("Loaded " + rows.size() + " rows");                // info level: not reported
        log.debug("constant " + "text");
        log.debug(PREFIX + "start");
        log.debug(String.format("%s", PREFIX));
        other.debug("x" + table);                                   // not a logger
        log.debug(() -> "lazy " + table);
    }

    void guarded(String table) {
        if (log.isDebugEnabled()) {
            log.debug("table=" + table);
        }
    }

    void earlyReturn(String table) {
        if (!log.isTraceEnabled()) {
            return;
        }
        log.trace("table=" + table);
    }

    void lambda(List<String> rows) {
        rows.forEach(r -> log.debug("row " + r));                   // expect: log-message-concatenation
    }
}

class Shadowed {
    void m(Printer log, String s) {
        log.debug("x" + s);                                         // a parameter that is not a logger
    }
}
