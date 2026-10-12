import java.util.ArrayList;
import java.util.List;
import java.util.Objects;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.function.Consumer;
import java.util.stream.Collectors;
import java.util.stream.Stream;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

class PeekSideEffect {
    private static final Logger LOG = LoggerFactory.getLogger(PeekSideEffect.class);

    static class Order {
        String status;
        boolean ready;
        void setStatus(String s) { status = s; }
        boolean isReady() { return ready; }
    }

    List<Order> mutate(List<Order> orders) {
        return orders.stream()
                .filter(Order::isReady)
                .peek(o -> o.setStatus("SENT"))                  // expect: peek-side-effect
                .collect(Collectors.toList());
    }

    long counted(List<String> names, List<String> seen, AtomicInteger n) {
        names.stream().peek(seen::add).count();                   // expect: peek-side-effect
        names.parallelStream().peek(s -> n.incrementAndGet()).forEach(s -> { });  // expect: peek-side-effect
        names.stream().map(String::trim).peek(Objects::requireNonNull).findFirst(); // expect: peek-side-effect
        return names.stream().peek(s -> {                          // expect: peek-side-effect
            LOG.debug("name {}", s);
            seen.add(s);
        }).count();
    }

    void onStreamVariable(Stream<String> s, List<String> sink) {
        s.peek(sink::add).forEach(x -> { });                       // expect: peek-side-effect
    }

    // Compliant
    List<String> debugging(List<String> names, Consumer<String> action) {
        names.stream().peek(System.out::println).count();
        names.stream().peek(s -> System.err.println("saw " + s)).count();
        names.stream().peek(s -> LOG.debug("saw {}", s)).count();
        names.stream().peek(LOG::info).count();
        names.stream().peek(s -> {
            if (LOG.isTraceEnabled()) {
                LOG.trace("saw {}", s);
            }
        }).count();
        names.stream().peek(s -> { }).count();
        names.stream().peek(action).count();                      // body not visible
        return names.stream().map(String::trim).collect(Collectors.toList());
    }

    static class Deque2 {
        String peek() { return null; }
        Deque2 peek(Consumer<String> c) { return this; }
    }

    void otherPeeks(java.util.Deque<String> d, Deque2 q, List<String> sink) {
        d.peek();                                                 // a queue, no argument
        q.peek(sink::add);                                        // not a stream
        new ArrayList<String>().iterator();
    }
}
