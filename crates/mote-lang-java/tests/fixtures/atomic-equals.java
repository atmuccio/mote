import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicLong;
import java.util.concurrent.atomic.AtomicReference;

class Retries {
    private final AtomicInteger retries = new AtomicInteger();
    private final AtomicInteger max = new AtomicInteger(3);
    private final AtomicLong total = new AtomicLong();
    private final AtomicBoolean done = new AtomicBoolean();

    boolean check(Integer limit, Object o) {
        if (retries.equals(max)) return true;                       // expect: atomic-equals
        if (total.equals(5L)) return true;                          // expect: atomic-equals
        if (this.done.equals(Boolean.TRUE)) return true;            // expect: atomic-equals
        return new AtomicInteger(1).equals(o);                      // expect: atomic-equals
    }

    boolean fine(Integer limit, AtomicReference<String> ref, Counter c) {
        return retries.get() == max.get()
                || limit.equals(retries.get())
                || ref.equals(ref)
                || c.equals(retries)
                || Integer.valueOf(3).equals(retries.get());
    }
}
