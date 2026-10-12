import java.util.Map;
import java.util.Objects;

class Settings {
    private Boolean enabled;
    private Boolean verbose;
    private boolean primitive;
    private final Map<String, Boolean> flags;

    Settings(Map<String, Boolean> flags) { this.flags = flags; }

    Boolean lookup(String key) { return flags.get(key); }

    void positives(Boolean param, Boolean[] many) {
        if (enabled) { start(); }                                   // expect: boxed-boolean-condition
        if (this.verbose) { start(); }                              // expect: boxed-boolean-condition
        while (param) { param = next(); }                           // expect: boxed-boolean-condition
        String s = flags.get("x") ? "a" : "b";                      // expect: boxed-boolean-condition
        if (!lookup("y")) { start(); }                              // expect: boxed-boolean-condition
        if (many[0]) { start(); }                                   // expect: boxed-boolean-condition
        do { start(); } while (flags.get("z"));                     // expect: boxed-boolean-condition
    }

    void negatives(Boolean checked, Boolean other, Unknown u) {
        if (primitive) { start(); }
        if (Boolean.TRUE.equals(enabled)) { start(); }
        if (Boolean.valueOf("true")) { start(); }
        if (Boolean.TRUE) { start(); }
        Boolean local = true;
        if (local) { start(); }                                     // initialized with a non-null value
        Boolean parsed = Boolean.valueOf("x");
        if (!parsed) { start(); }
        if (checked != null && checked) { start(); }
        if (checked) { start(); }                                   // null-checked in this method
        if (Objects.nonNull(other)) {
            if (other) { start(); }
        }
        if (u.flag()) { start(); }                                  // unknown type
        Object o = u.value();
        if (o instanceof Boolean b && b) { start(); }
        if (o instanceof Boolean flag) {
            String s = flag ? "Y" : "N";                            // a pattern variable is never null
        }
        if (enabled == null || !enabled.booleanValue()) { start(); }
        boolean b = primitive && !primitive;
    }

    Boolean next() { return Boolean.FALSE; }

    void start() { }
}
