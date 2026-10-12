import java.util.ArrayList;
import java.util.Collection;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.Set;
import java.util.function.Supplier;

class Repository {
    List<String> names(boolean ok) {
        if (!ok) {
            return null;                                            // expect: collection-return-null
        }
        return new ArrayList<>();
    }

    Map<String, Integer> counts() {
        return null;                                                // expect: collection-return-null
    }

    String[] columns() {
        return (null);                                              // expect: collection-return-null
    }

    byte[] payload() {
        return null;                                                // binary data: null means "no value"
    }

    char[] password() {
        return null;
    }

    ArrayList<String> concrete() {
        return null;                                                // expect: collection-return-null
    }

    Set<String>[] arrayOfSets() {
        return null;                                                // expect: collection-return-null
    }

    String text() {
        return null;
    }

    Optional<String> maybe() {
        return Optional.empty();
    }

    Iterable<String> iterable() {
        return null;                                                // Iterable is not reported (Path is Iterable)
    }

    Unknown unknown() {
        return null;
    }

    @Nullable
    List<String> documented() {
        return null;
    }

    Supplier<List<String>> supplier() {
        return () -> {
            return null;                                            // the lambda's return, not the method's
        };
    }

    List<String> withLambda() {
        Runnable r = () -> {
            return;
        };
        Supplier<Object> s = () -> {
            return null;                                            // inside a lambda
        };
        return new ArrayList<>();
    }

    Collection<String> anonymous() {
        Object o = new Object() {
            String inner() {
                return null;
            }
        };
        return List.of();
    }
}
