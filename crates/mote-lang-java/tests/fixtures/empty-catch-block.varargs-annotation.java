import org.jspecify.annotations.Nullable;

// A type annotation on the ellipsis (valid since Java 8) must not break the parse: the findings
// below keep their lines and columns.
class Varargs {
    static String join(String @Nullable ... parts) {
        try {
            return String.join(",", parts);
        } catch (RuntimeException e) { }                          // expect: empty-catch-block
    }

    static void log(Object @org.jspecify.annotations.NonNull... values) {
        try { run(values); } catch (IllegalStateException e) { }   // expect: empty-catch-block
    }

    static void run(Object... values) { }
}
