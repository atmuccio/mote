import java.util.List;
import java.util.Optional;
import java.util.OptionalInt;
import java.util.OptionalDouble;

class Optionals {
    private Optional<String> cached;

    String positives(Optional<String> name, OptionalInt count, OptionalDouble avg, List<Optional<String>> list) {
        String a = name.get();                          // expect: optional-get-without-check
        int b = count.getAsInt();                       // expect: optional-get-without-check
        double c = avg.getAsDouble();                   // expect: optional-get-without-check
        String d = cached.get();                        // expect: optional-get-without-check
        Optional<String> found = list.get(0);
        return a + b + c + d + found.get();             // expect: optional-get-without-check
    }

    String checkedLater(Optional<String> name) {
        String v = name.get();                          // expect: optional-get-without-check
        if (name.isPresent()) {
            return v;
        }
        return "";
    }

    String negatives(Optional<String> name, Optional<String> other, OptionalInt count, Unknown u) {
        if (name.isPresent()) {
            System.out.println(name.get());
        }
        if (other.isEmpty()) {
            return "";
        }
        String o = other.get();
        int n = count.isPresent() ? count.getAsInt() : 0;
        String t = this.cached.isPresent() ? this.cached.get() : cached.get();
        String x = u.opt().get();                       // not a variable
        String y = u.value.get();                       // unknown type
        Optional<String> fixed = Optional.of("x");
        String z = fixed.get();                         // Optional.of can't be empty
        name.ifPresent(v -> System.out.println(name.get()));
        String w = name.orElse("none");
        List<String> l = List.of("a");
        String first = l.get(0);                        // not an Optional
        return o + n + t + x + y + z + w + first;
    }

    Runnable lambdas(Optional<String> name) {
        if (!name.isPresent()) {
            return () -> {};
        }
        return () -> System.out.println(name.get());   // checked earlier in the method
    }

    Runnable anonymous(Optional<String> name) {
        if (name.isPresent()) {
            return new Runnable() {
                @Override
                public void run() {
                    System.out.println(name.get());     // checked around the anonymous class
                }
            };
        }
        return null;
    }

    static class Holder {
        private Optional<String> value;

        void check() {
            if (value.isPresent()) { use(); }
        }

        String read() {
            return value.get();                         // expect: optional-get-without-check
        }

        void use() {}
    }

    String otherMethod(Optional<String> name) {
        return name.get();                              // expect: optional-get-without-check
    }
}
