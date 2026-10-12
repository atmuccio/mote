import java.io.InputStream;
import java.io.Reader;

// Annotations and `final` on a `case` type pattern (valid since Java 21) must not break the
// parse: the findings inside and after the switch keep their lines and columns.
class Sources {
    static int open(Object source) {
        switch (source) {
            case @SuppressWarnings("unused") InputStream in -> {
                try { mark(); } catch (IllegalStateException e) { }   // expect: empty-catch-block
                return 1;
            }
            case final Reader reader -> {
                return reader.hashCode();
            }
            case @SuppressWarnings({"unused", "rawtypes"})
                    final java.util.List list -> {
                return 2;
            }
            default -> {
                return 0;
            }
        }
    }

    static String describe(Object value) {
        String kind = switch (value) {
            case final Integer i when i > 0 -> "positive";
            case @Deprecated String s -> "text";
            default -> "other";
        };
        try {
            mark();
        } catch (RuntimeException e) { }                          // expect: empty-catch-block
        return kind;
    }

    static void mark() { }
}
