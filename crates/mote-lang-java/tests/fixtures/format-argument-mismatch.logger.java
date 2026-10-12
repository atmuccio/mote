import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

class Service {
    private static final Logger LOG = LoggerFactory.getLogger(Service.class);
    private final org.apache.logging.log4j.Logger log4j = null;

    void positives(int attempt, int max, String user) {
        LOG.warn("Retry {} of {} failed", attempt);                         // expect: format-argument-mismatch
        LOG.info("User {} logged in", user, attempt);                       // expect: format-argument-mismatch
        LOG.debug("No placeholders", user);                                 // expect: format-argument-mismatch
        LOG.error("Value {}");                                              // expect: format-argument-mismatch
        log4j.info("{} {} {}", user, max);                                  // expect: format-argument-mismatch
        log4j.error("Invalid CRC %08X for %s", attempt, user);              // expect: format-argument-mismatch
        try {
            run();
        } catch (IllegalStateException e) {
            LOG.error("Failed {} {}", user, e);                             // counts agree
            LOG.error("Failed {} {} {}", user, e);                          // expect: format-argument-mismatch
        }
    }

    void negatives(int attempt, String user, Object[] args, Unknown u, Throwable cause) {
        LOG.info("Retry {} of {}", attempt, 3);
        LOG.info("plain message");
        LOG.info("literal \\{} and {}", user);                               // escaped placeholder
        try {
            run();
        } catch (RuntimeException e) {
            LOG.error("Failed for {}", user, e);                            // trailing exception
            LOG.error("Failed", e);
        }
        LOG.warn("Failed for {}", user, cause);
        LOG.warn("Several {} {}", args);                                    // the varargs array itself
        LOG.warn("Unknown {}", user, u.problem());                          // may be an exception
        u.info("{} {}", user);                                              // unknown logger type
        LOG.info("{} {}", user, attempt);
    }

    void run() { }
}

@lombok.extern.slf4j.Slf4j
class Generated {
    void run(String a) {
        log.info("{} and {}", a);                                           // expect: format-argument-mismatch
    }
}
