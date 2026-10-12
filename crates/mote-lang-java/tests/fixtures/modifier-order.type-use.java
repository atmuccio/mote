// mote-test: params type_use_annotations=true
import org.jspecify.annotations.Nullable;
import org.checkerframework.checker.nullness.qual.NonNull;
import javax.annotation.CheckForNull;

class ModifierOrderTypeUse {
    @Nullable private String name;                            // expect: modifier-order
    private @Nullable String fine;
    @NonNull public static String NAME = "n";                 // expect: modifier-order
    @CheckForNull private String declarationAnnotation;

    @Nullable public String find() {                          // expect: modifier-order
        return null;
    }

    public @Nullable String findFine() {
        return null;
    }

    @Override @Nullable public String toString() {            // expect: modifier-order
        return null;
    }

    @Nullable String noKeywords() {
        return null;
    }

    static @Nullable final String ORDER = null;               // expect: modifier-order
}
