import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

public class Constants {
    public static final String[] LEVELS = {"low", "high"};          // expect: public-static-mutable-field
    public static final int[] PRIMES = new int[] {2, 3, 5};         // expect: public-static-mutable-field
    public static final byte[] BUFFER = new byte[16];               // expect: public-static-mutable-field
    public static final String NAMES[] = {"a"};                     // expect: public-static-mutable-field
    public static final String[] EMPTY = {};
    public static final Object[] NONE = new Object[0];
    public static final int[] NOTHING = new int[] {};
    private static final String[] HIDDEN = {"x"};
    static final String[] PACKAGE = {"x"};

    public static final List<String> NAMES_LIST = new ArrayList<>();                     // expect: public-static-mutable-field
    public static final Map<String, Integer> CODES = new HashMap<>();                    // expect: public-static-mutable-field
    public static final Map<String, Object> REGISTRY = new ConcurrentHashMap<>();        // expect: public-static-mutable-field
    public static final List<String> FIXED = Arrays.asList("a", "b");                    // expect: public-static-mutable-field
    public static final List<String> SAFE = List.of("a");
    public static final List<String> WRAPPED = Collections.unmodifiableList(new ArrayList<>());
    public static final List<String> FROM_FACTORY = Factory.names();
    public static final Unknown CUSTOM = new Unknown();
    public static final String TEXT = "x";

    @Deprecated
    public static final String[] ANNOTATED = {"x"};

    public interface Keys {
        String[] ALL = {"a", "b"};                                   // expect: public-static-mutable-field
        String ONE = "a";
        List<String> MORE = new ArrayList<>();                      // expect: public-static-mutable-field
    }

    private interface HiddenKeys {
        String[] ALL = {"a"};
    }

    public enum Level {
        LOW, HIGH;
        public int hits;                                            // expect: public-static-mutable-field
        public final int weight = 1;
        private String label;
        int packageField;
        public static final String[] NAMES = {"low"};               // expect: public-static-mutable-field
    }

    private enum HiddenLevel {
        A;
        public int hits;
    }
}
