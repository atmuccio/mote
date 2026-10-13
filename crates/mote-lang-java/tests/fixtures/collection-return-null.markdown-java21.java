// mote-test: params java_release=21
// A project compiled for Java 21 may be documented with `///` runs (the JDK running javadoc
// decides): a documented `null` result counts on any release.
import java.util.ArrayList;
import java.util.List;

class Matcher {
    /// @return the matched variables, or `null` if it does not match
    public List<String> match(String data) {
        if (data.isEmpty()) {
            return null;
        }
        return new ArrayList<>();
    }

    /// @return the values, never empty
    public List<String> values() {
        return null;                                                // expect: collection-return-null
    }

    /** @return the names, or null when unknown */
    public List<String> names() {
        return null;
    }
}
