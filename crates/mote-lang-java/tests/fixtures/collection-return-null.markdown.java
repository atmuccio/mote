// mote-test: params java_release=23
import java.util.ArrayList;
import java.util.List;

class Matcher {
    /// Matches the pattern.
    ///
    /// @param data the input
    /// @return the matched variables, or `null` if it does not match
    public List<String> match(String data) {
        if (data.isEmpty()) {
            return null;
        }
        return new ArrayList<>();
    }

    /// @return the names;
    ///     null when unknown
    /// @throws IllegalStateException never
    public List<String> names() {
        return null;
    }

    /// Returns the values, never null.
    ///
    /// @param key the key
    /// @return the values
    /// @throws IllegalArgumentException if the key is null
    public List<String> values(String key) {
        return null;                                                // expect: collection-return-null
    }

    //// @return null when absent (four slashes: not documentation)
    public List<String> banner() {
        return null;                                                // expect: collection-return-null
    }

    /// @return null when absent, in a run the blank line ends

    /// The nearest run documents the method.
    public List<String> separated() {
        return null;                                                // expect: collection-return-null
    }
}
