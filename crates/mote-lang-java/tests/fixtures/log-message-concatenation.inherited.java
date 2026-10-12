import org.apache.commons.logging.Log;
import org.apache.commons.logging.LogFactory;

class Child extends UnknownBase {
    void m(String s) {
        log.debug("value " + s);                                    // expect: log-message-concatenation
        logger.trace("value " + s);                                 // expect: log-message-concatenation
        audit.debug("value " + s);                                  // unconventional name, unknown type
    }
}
