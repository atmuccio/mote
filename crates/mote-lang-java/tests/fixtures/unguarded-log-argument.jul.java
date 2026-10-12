import java.util.logging.Level;
import java.util.logging.Logger;

class Bridge {
    private static final Logger LOGGER = Logger.getLogger(Bridge.class.getName());

    void run(Link link, Exception e) {
        LOGGER.fine("Link " + link.describe());                     // java/log-message-concatenation
        LOGGER.finer(link.describe());                              // expect: unguarded-log-argument
        LOGGER.log(Level.FINE, "Link " + link.describe());          // expect: unguarded-log-argument
        LOGGER.log(Level.FINEST, "Link {0}", link.describe());      // expect: unguarded-log-argument

        // Not reported.
        LOGGER.log(Level.INFO, "Link " + link.describe());
        LOGGER.log(Level.FINE, "Link {0}", link);
        LOGGER.fine(() -> "Link " + link.describe());
        LOGGER.info(link.describe());
        LOGGER.log(Level.WARNING, "Failed", e);
        if (LOGGER.isLoggable(Level.FINE)) {
            LOGGER.fine(link.describe());
        }
    }

    interface Link {
        String describe();
    }
}
