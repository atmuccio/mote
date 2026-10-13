// Shapes the previous grammar rejected: type annotations before a varargs ellipsis (Java 8),
// annotations and `final` on type patterns (Java 21), qualified record patterns, annotated
// `instanceof` types.
import java.util.Collection;

class AnnotatedPatterns {
    @interface Nullable {}

    record Shelf(int width) {
        record Slot(int index) {}
    }

    static void note(int level, @Nullable Object @Nullable ... values) {}

    static int classify(Object o) {
        return switch (o) {
            case @SuppressWarnings("unused") String s -> 1;
            case final Integer i when i > 0 -> i;
            case Shelf.Slot(int index) -> index;
            default -> 0;
        };
    }

    static boolean isEmptyish(Object o) {
        return o instanceof @SuppressWarnings("unchecked") Collection values && !values.isEmpty()
                || o instanceof final String s && s.isEmpty();
    }
}
