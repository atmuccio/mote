import java.util.Map;
import java.util.Objects;
import java.util.Optional;

class Fallbacks {
    private Map<String, Object> context;
    private Map<String, Boolean> flags;

    void assignedOnEveryPath(Map<String, Object> ctx) {
        Boolean reloaded;
        if (ctx != null) {
            reloaded = (Boolean) ObjectUtils.defaultIfNull(ctx.get("r"), Boolean.FALSE);
        } else {
            reloaded = Boolean.FALSE;
        }
        if (!reloaded) {
            clear();
        }
    }

    void oneNullableAssignment(Map<String, Boolean> m) {
        Boolean found;
        if (m.isEmpty()) {
            found = Boolean.TRUE;
        } else {
            found = m.get("k");
        }
        if (found) {                                                // expect: boxed-boolean-condition
            clear();
        }
    }

    String optionalFallback(Optional<Boolean> active) {
        return active.orElse(false) ? "on" : "off";
    }

    String requireElse(Boolean maybe) {
        Boolean value = Objects.requireNonNullElse(flags.get("x"), Boolean.TRUE);
        return Objects.requireNonNullElse(flags.get("y"), true) ? "a" : "b";
    }

    String optionalNullFallback(Optional<Boolean> active) {
        return active.orElse(null) ? "on" : "off";                  // expect: boxed-boolean-condition
    }

    String unknownFallback(Optional<Boolean> active) {
        return active.orElse(flags.get("z")) ? "on" : "off";        // expect: boxed-boolean-condition
    }

    void clear() { }
}
