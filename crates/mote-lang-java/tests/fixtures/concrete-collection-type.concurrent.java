import java.util.ArrayDeque;
import java.util.Hashtable;
import java.util.IdentityHashMap;
import java.util.PriorityQueue;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.ConcurrentLinkedQueue;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.concurrent.DelayQueue;
import java.util.concurrent.LinkedBlockingQueue;

class Registry {
    private final ConcurrentHashMap<String, Integer> counts = new ConcurrentHashMap<>();   // expect: concrete-collection-type
    private final CopyOnWriteArrayList<Runnable> hooks = new CopyOnWriteArrayList<>();     // expect: concrete-collection-type
    private final ConcurrentLinkedQueue<String> events = new ConcurrentLinkedQueue<>();    // expect: concrete-collection-type
    private final ArrayDeque<String> stack = new ArrayDeque<>();                           // expect: concrete-collection-type
    private final LinkedBlockingQueue<String> work = new LinkedBlockingQueue<>();          // expect: concrete-collection-type
    private final PriorityQueue<Integer> heap = new PriorityQueue<>();                     // expect: concrete-collection-type

    private final ConcurrentHashMap<String, Integer> sized = new ConcurrentHashMap<>();    // mappingCount() is its own
    private final ConcurrentHashMap<String, Integer> parallel = new ConcurrentHashMap<>(); // forEach(parallelism, …)
    private final ConcurrentHashMap<String, Integer> sequential = new ConcurrentHashMap<>(); // expect: concrete-collection-type
    private final CopyOnWriteArrayList<String> unique = new CopyOnWriteArrayList<>();      // addIfAbsent() is its own
    private final PriorityQueue<Integer> ordered = new PriorityQueue<>();                  // comparator() is its own
    private final Hashtable<String, String> legacy = new Hashtable<>();                    // contains(value) is its own
    private final IdentityHashMap<Object, String> ids = new IdentityHashMap<>();           // identity semantics: the point
    private final DelayQueue<java.util.concurrent.Delayed> delays = new DelayQueue<>();

    void use(String s) {
        counts.merge(s, 1, Integer::sum);
        hooks.add(() -> { });
        events.offer(s);
        String head = events.poll();
        stack.push(s);
        stack.pop();
        work.offer(s);
        heap.offer(1);
        heap.poll();
        long n = sized.mappingCount();
        parallel.forEach(4, (k, v) -> System.out.println(k));
        sequential.forEach((k, v) -> System.out.println(k));
        unique.addIfAbsent(s);
        ordered.comparator();
        boolean has = legacy.contains("x");
    }

    ConcurrentHashMap<String, Integer> snapshot() {                                        // expect: concrete-collection-type
        return counts;
    }

    void drain(ArrayDeque<String> queue) {                                                 // expect: concrete-collection-type
        queue.pollFirst();
    }
}
