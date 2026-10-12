import java.util.Optional;
import java.util.OptionalInt;

class Users {
    private Optional<String> cached = null;                     // expect: optional-null
    private Optional<String> ok = Optional.empty();

    Optional<String> find(String id) { return Optional.ofNullable(id); }

    void positives(Optional<String> lookup, OptionalInt count) {
        if (lookup != null && lookup.isPresent()) {             // expect: optional-null
            use(lookup.get());
        }
        if (null == count) {                                    // expect: optional-null
            return;
        }
        Optional<String> user = null;                           // expect: optional-null
        cached = null;                                          // expect: optional-null
        if (find("x") == null) {                                // expect: optional-null
            return;
        }
    }

    void negatives(String name, Object o, Optional<String> lookup) {
        if (name != null) {
            use(name);
        }
        String s = null;
        name = null;
        if (lookup.isPresent()) {
            use(lookup.get());
        }
        Runnable r = () -> {};
        if (unknown() == null) {
            return;
        }
    }

    native Object unknown();
    void use(String s) {}
}
