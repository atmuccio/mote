import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

class Views {
    private final Map<String, Integer> sizes = new ConcurrentHashMap<>();

    String[] keys(Map<String, Integer> m) {
        String[] a = m.keySet().toArray(new String[m.size()]);                  // expect: presized-to-array
        Integer[] b = sizes.values().toArray(new Integer[sizes.size()]);        // expect: presized-to-array
        Object[] c = m.entrySet().toArray(new Object[m.size()]);                // expect: presized-to-array

        String[] ok1 = m.keySet().toArray(new String[0]);
        String[] ok2 = m.keySet().toArray(new String[sizes.size()]);            // another map
        String[] ok3 = m.keySet().toArray(new String[m.size() + 1]);
        String[] ok4 = unknown.keySet().toArray(new String[unknown.size()]);    // type unknown (a multimap?)
        return a;
    }

    Multimap unknown;
}
