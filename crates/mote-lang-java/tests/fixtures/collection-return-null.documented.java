import java.util.ArrayList;
import java.util.List;

class Matcher {
    /**
     * Matches the pattern.
     *
     * @param data the input
     * @return The list of matched variables, or <code>null</code> if it does not match.
     */
    public List<String> match(String data) {
        if (data.isEmpty()) {
            return null;
        }
        return new ArrayList<>();
    }

    /**
     * @return the names; {@code null} when unknown
     * @throws IllegalStateException never
     */
    public List<String> names() {
        return null;
    }

    /**
     * Returns the values, never null.
     *
     * @param key the key
     * @return the values
     * @throws IllegalArgumentException if the key is null
     */
    public List<String> values(String key) {
        return null;                                                // expect: collection-return-null
    }

    /** @return the nullable-looking word annulled does not count */
    public List<String> words() {
        return null;                                                // expect: collection-return-null
    }

    // @return null when absent (not Javadoc)
    public List<String> lineComment() {
        return null;                                                // expect: collection-return-null
    }
}
