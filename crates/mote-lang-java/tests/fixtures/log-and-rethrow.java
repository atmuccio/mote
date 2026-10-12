import java.io.IOException;
import java.io.UncheckedIOException;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

class Orders {
    private static final Logger log = LoggerFactory.getLogger(Orders.class);

    void positives(Store store, Order order) throws IOException {
        try {
            store.save(order);
        } catch (IOException e) {
            log.error("could not save order {}", order.id(), e);           // expect: log-and-rethrow
            throw e;
        }
        try {
            store.save(order);
        } catch (IOException e) {
            log.warn("save failed: " + e.getMessage());                     // expect: log-and-rethrow
            throw new UncheckedIOException("save failed", e);
        }
        try {
            store.save(order);
        } catch (RuntimeException ex) {
            if (order.isUrgent()) {
                log.info("urgent order failed", ex);                        // conditional branch
                throw new IllegalStateException(ex);
            }
        }
        try {
            store.save(order);
        } catch (IOException e) {
            // context first
            log.error("failed", e);                                          // expect: log-and-rethrow
            metrics.increment();
            throw new StorageException(
                "could not save", e);
        }
        try {
            store.save(order);
        } catch (IOException e) {
            this.log.error("failed", e);                                     // expect: log-and-rethrow
            throw (e);
        }
    }

    void negatives(Store store, Order order) throws Exception {
        // Logged and handled.
        try {
            store.save(order);
        } catch (IOException e) {
            log.error("could not save order", e);
            order.markFailed();
        }
        // Rethrown without logging.
        try {
            store.save(order);
        } catch (IOException e) {
            throw new StorageException("could not save", e);
        }
        // Debug output is diagnostics.
        try {
            store.save(order);
        } catch (IOException e) {
            log.debug("save failed", e);
            throw e;
        }
        // The rethrow happens only on some paths.
        try {
            store.save(order);
        } catch (IOException e) {
            log.warn("retrying", e);
            if (order.attempts() > 3) {
                throw e;
            }
        }
        // A different exception, without the cause (another rule's business).
        try {
            store.save(order);
        } catch (IOException e) {
            log.error("failed", e);
            throw new IllegalStateException("gave up");
        }
        // Logging something unrelated to the exception.
        try {
            store.save(order);
        } catch (IOException e) {
            log.error("order {} could not be saved", order.id());
            throw e;
        }
        // Not a logger.
        try {
            store.save(order);
        } catch (IOException e) {
            audit.error("failed", e);
            throw e;
        }
        // The throw sits in a nested try with its own handler.
        try {
            store.save(order);
        } catch (IOException e) {
            try {
                log.error("failed", e);
                throw e;
            } catch (IOException again) {
                order.markFailed();
            }
        }
        // Inside a lambda.
        try {
            store.save(order);
        } catch (IOException e) {
            Runnable r = () -> {
                log.error("failed", e);
                throw new UncheckedIOException(e);
            };
            r.run();
        }
        // The log call comes after the throw in another block.
        try {
            store.save(order);
        } catch (IOException e) {
            if (order.isUrgent()) {
                throw e;
            }
            log.error("failed", e);
        }
    }

    private Metrics metrics;
    private Audit audit;
}
