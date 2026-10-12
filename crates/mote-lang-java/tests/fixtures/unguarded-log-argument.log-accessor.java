import java.util.List;
import org.springframework.core.log.LogAccessor;

class Listener {
    private static final LogAccessor LOG = new LogAccessor(Listener.class);
    private static final String PREFIX = "p";

    void handle(Object message, List<String> ids, Exception ex) {
        LOG.debug("Processing [" + message + "]");                 // expect: unguarded-log-argument
        LOG.trace(String.format("Ids %s", ids));                   // expect: unguarded-log-argument
        LOG.debug(ex, "Failed for " + ids.size());                 // expect: unguarded-log-argument
        LOG.debug(render(message));                                // expect: unguarded-log-argument

        LOG.debug(() -> "Processing [" + message + "]");           // a supplier: lazy
        LOG.debug(ex, () -> "Failed for " + ids.size());
        LOG.debug("Constant " + PREFIX);                           // constants only
        LOG.debug(ex, "Consumer stopped");
        LOG.info("Processing [" + message + "]");                  // an enabled level
        if (LOG.isDebugEnabled()) {
            LOG.debug("Guarded " + message);
        }
    }

    String render(Object o) {
        return String.valueOf(o);
    }
}
