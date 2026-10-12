import java.io.File;
import java.util.Iterator;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.atomic.AtomicInteger;

class Jobs {
    private final AtomicInteger counter = new AtomicInteger();
    private int seen;

    void wrong(Set<String> pending, String job, Iterator<String> it, Map<String, Integer> m, File f, int x) {
        assert pending.remove(job) : "job was not pending";             // expect: assert-with-side-effect
        assert it.next() != null;                                       // expect: assert-with-side-effect
        assert counter.incrementAndGet() > 0;                           // expect: assert-with-side-effect
        assert m.put(job, 1) == null;                                   // expect: assert-with-side-effect
        assert f.delete();                                              // expect: assert-with-side-effect
        assert seen++ < 10;                                             // expect: assert-with-side-effect
        assert (x = 3) > 0;                                             // expect: assert-with-side-effect
    }

    void fine(Set<String> pending, String job, List<String> list, Registry registry, Map<String, Integer> m) {
        boolean removed = pending.remove(job);
        assert removed : "job was not pending";
        assert pending.contains(job);
        assert list.size() > 0 : "size " + list.remove(0);
        assert registry.remove(job);
        assert m.get(job) != null;
        assert list.stream().filter(s -> { int y = 0; y++; return y > 0; }).count() > 0;
        assert counter.get() >= 0;
        assert seen + 1 > 0;
    }

    interface Registry { boolean remove(String s); }
}
