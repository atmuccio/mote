import java.time.Duration;
import java.util.Arrays;
import java.util.List;
import java.util.Map;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

class OrderService {
    private static final Logger log = LoggerFactory.getLogger(OrderService.class);
    private static final String PREFIX = "order-";
    private final Mapper mapper = new Mapper();

    void positives(Order order, List<String> ids, Map<String, Order> byId, long elapsed, int[] codes) {
        log.debug("Order {}", mapper.toJson(order));               // expect: unguarded-log-argument
        log.trace("Took {}", Duration.ofMillis(elapsed));          // expect: unguarded-log-argument
        log.debug("Codes {}", Arrays.toString(codes));             // expect: unguarded-log-argument
        log.debug("Ids {}", String.join(",", ids));                // expect: unguarded-log-argument
        log.debug("State {}", order.toString());                   // expect: unguarded-log-argument
        log.debug("Snapshot {}", new Snapshot(order));             // expect: unguarded-log-argument
        log.debug("Key {}", PREFIX + order.getId());               // expect: unguarded-log-argument
        log.debug(mapper.toJson(order));                           // expect: unguarded-log-argument
        log.debug("Summary {} {}", order.getId(), order.summarize()); // expect: unguarded-log-argument
        log.trace("First {}", byId.get(ids.get(0)).describe());    // expect: unguarded-log-argument
        log.debug("Flag {}", order.isOpen() ? order.render() : "-"); // expect: unguarded-log-argument
        log.debug("Items {}", (Object) order.items().toArray());   // expect: unguarded-log-argument
    }

    void negatives(Order order, List<String> ids, Map<String, Order> byId, Exception e, long elapsed) {
        log.debug("Order {}", order);
        log.debug("Order {} has {} items", order.getId(), ids.size());
        log.debug(e.getMessage(), e);
        log.debug("Lookup {}", byId.get(order.getId()));
        log.debug("Thread {}", Thread.currentThread().getName());
        log.debug("Elapsed {} ms", System.currentTimeMillis() - elapsed);
        log.debug("Constant " + PREFIX);
        log.debug("Built " + order.summarize());                    // java/log-message-concatenation
        log.debug(String.format("Order %s", order.summarize()));   // java/log-message-concatenation
        log.info("Order {}", mapper.toJson(order));                // info level: not reported
        log.warn("Order {}", order.summarize());
        log.error("Failed {}", order.summarize(), e);
        log.debug("Order {}", (Object) order);
        log.debug("Many {} {}", new Object[] {order, ids});
        log.debug("Id {}", order.id());
        if (log.isDebugEnabled()) {
            log.debug("Order {}", mapper.toJson(order));
        }
        log.debug("Order {}", () -> mapper.toJson(order));
        log.debug("Order {}", mapper::summary);
        Runnable r = () -> log.debug("x {}", order.getId());
    }

    void earlyReturn(Order order) {
        if (!log.isTraceEnabled()) {
            return;
        }
        log.trace("Order {}", mapper.toJson(order));
    }

    private static final boolean DEBUG_ON = log.isDebugEnabled();

    void flag(Order order) {
        if (DEBUG_ON) {
            log.debug("Order {}", mapper.toJson(order));
        }
        boolean traceOn = false;
        if (traceOn && order != null) {
            log.trace("Order {}", mapper.toJson(order));
        }
    }

    void notALogger(Printer printer, Order order) {
        printer.debug("Order {}", mapper.toJson(order));
    }

    static class Mapper {
        String toJson(Object o) {
            return "";
        }

        String summary() {
            return "";
        }
    }

    interface Printer {
        void debug(String format, Object arg);
    }

    record Snapshot(Order order) {
    }

    interface Order {
        String getId();

        String id();

        boolean isOpen();

        String summarize();

        String describe();

        String render();

        List<String> items();
    }
}
