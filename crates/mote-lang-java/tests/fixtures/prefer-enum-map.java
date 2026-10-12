import java.util.*;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.TimeUnit;

class Colors {
    enum Color { RED, GREEN, BLUE }

    Map<Color, Integer> counts = new HashMap<>();                   // expect: prefer-enum-map
    Map<Color, String> names = new HashMap<Color, String>(16);      // expect: prefer-enum-map
    SortedMap<Color, String> sorted = new TreeMap<>();             // expect: prefer-enum-map
    Set<Color> seen = new HashSet<>();                             // expect: prefer-enum-map
    Set<TimeUnit> units = new TreeSet<TimeUnit>();                 // expect: prefer-enum-map

    void local() {
        Map<Color, List<String>> byColor;
        byColor = new HashMap<>();                                 // expect: prefer-enum-map
        Set<Color> other = new HashSet<Color>();                   // expect: prefer-enum-map
    }

    // Fine: insertion order, concurrency, custom order, copies, other keys, anonymous subclasses.
    Map<Color, Integer> ordered = new LinkedHashMap<>();
    Map<Color, Integer> shared = new ConcurrentHashMap<>();
    Map<Color, Integer> custom = new TreeMap<>(Comparator.reverseOrder());
    Set<Color> copy = new HashSet<>(Arrays.asList(Color.values()));
    Map<String, Color> byName = new HashMap<>();
    Map<Level, Integer> external = new HashMap<>();
    Map<Color, Integer> anon = new HashMap<>() {
        { put(Color.RED, 1); }
    };
    Map raw = new HashMap();
    var inferred = new HashMap<Color, Integer>();                  // expect: prefer-enum-map
    EnumMap<Color, Integer> good = new EnumMap<>(Color.class);

    <K> Map<K, Integer> generic() {
        return new HashMap<K, Integer>();
    }

    Map<Color, Integer> returned() {
        return new HashMap<>();
    }
}
