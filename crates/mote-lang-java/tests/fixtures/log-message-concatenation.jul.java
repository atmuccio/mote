import java.util.logging.Level;
import java.util.logging.Logger;

class JulUser {
    private static final Logger LOGGER = Logger.getLogger("x");

    void m(String s) {
        LOGGER.fine("value " + s);                                  // expect: log-message-concatenation
        LOGGER.finest("value " + s);                                // expect: log-message-concatenation
        LOGGER.info("value " + s);
        LOGGER.debug("value " + s);                                 // not a JUL method name
        LOGGER.log(Level.FINE, "value {0}", s);
    }

    void guarded(String s) {
        if (LOGGER.isLoggable(Level.FINE)) {
            LOGGER.fine("value " + s);
        }
    }
}
