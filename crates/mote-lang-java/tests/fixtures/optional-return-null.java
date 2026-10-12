import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.OptionalInt;
import java.util.OptionalLong;
import java.util.function.Function;
import java.util.function.Supplier;

class Finder {
    private Map<String, String> users;

    Optional<String> find(String id) {
        if (id == null) {
            return null;                                    // expect: optional-return-null
        }
        return Optional.ofNullable(users.get(id));
    }

    OptionalInt count(String id) {
        return (null);                                      // expect: optional-return-null
    }

    public static java.util.Optional<List<String>> all() {
        return null;                                        // expect: optional-return-null
    }

    OptionalLong size() {
        return id() > 0 ? OptionalLong.of(1) : null;        // only a plain `return null` is reported
    }

    String negatives(String id) {
        if (id == null) {
            return null;                                    // not an Optional method
        }
        return id;
    }

    Optional<String> lambdas() {
        Supplier<String> s = () -> { return null; };        // returns from the lambda
        Function<String, String> f = new Function<>() {
            @Override
            public String apply(String x) { return null; }  // anonymous class method
        };
        return Optional.empty();
    }

    Optional<String> ok() {
        return Optional.empty();
    }

    int id() { return 1; }

    static class Own {
        static class Optional<T> {}

        Optional<String> mine() {
            return null;                                    // this file's own Optional class
        }
    }
}
