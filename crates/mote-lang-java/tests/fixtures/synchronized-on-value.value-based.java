import java.time.Instant;
import java.time.LocalDate;
import java.util.Optional;

class Locks {
    private final Optional<String> maybe = Optional.empty();
    private final LocalDate day = LocalDate.now();
    private final Object lock = new Object();

    void positives(Instant when) {
        synchronized (maybe) {                  // expect: synchronized-on-value
            touch();
        }
        synchronized (this.day) {               // expect: synchronized-on-value
            touch();
        }
        synchronized (when) {                   // expect: synchronized-on-value
            touch();
        }
    }

    void negatives(java.util.List<String> list, Holder h) {
        synchronized (lock) {
            touch();
        }
        synchronized (list) {
            touch();
        }
        synchronized (h) {
            touch();
        }
    }

    static class Holder {}

    void touch() {}
}
