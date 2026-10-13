import java.util.Objects;

// Java 25 lets statements run before `super(...)`: a Runnable handed to Thread there still
// counts, wherever the call sits in the constructor.
class Workers {
    final class Checked extends Thread {
        Checked(Runnable task) {
            Objects.requireNonNull(task, "task");
            super(task);
        }
    }

    final class Named extends Thread {                               // expect: thread-subclass-without-run
        Named(String name) {
            String trimmed = name.strip();
            super(trimmed);
        }
    }
}
