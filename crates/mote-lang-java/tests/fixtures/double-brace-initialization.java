import java.util.ArrayList;
import java.util.HashMap;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

class Limits {
    Map<String, Integer> limits = new HashMap<>() {{                // expect: double-brace-initialization
        put("users", 100);
        put("roles", 10);
    }};

    List<String> names() {
        return new ArrayList<String>() {                            // expect: double-brace-initialization
            private static final long serialVersionUID = 1L;
            {
                add("a");
            }
        };
    }

    Object set = new java.util.HashSet<String>() {{ add("x"); }};   // expect: double-brace-initialization

    Map<String, String> lru = new LinkedHashMap<>(16, 0.75f, true) {
        @Override
        protected boolean removeEldestEntry(Map.Entry<String, String> e) {
            return size() > 100;
        }
    };

    Map<String, String> mixed = new HashMap<>() {
        {
            put("a", "b");
        }

        @Override
        public String get(Object k) {
            return super.get(k);
        }
    };

    Object thread = new Thread() {{ setDaemon(true); }};            // not a collection
    Object unknown = new Registry() {{ register("x"); }};           // unknown type
    Map<String, Integer> plain = new HashMap<>();
}
