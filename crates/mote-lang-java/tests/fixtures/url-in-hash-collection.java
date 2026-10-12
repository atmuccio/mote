import java.net.URI;
import java.net.URL;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.TreeMap;
import java.util.concurrent.ConcurrentHashMap;

class Crawler {
    private final Map<URL, String> cache = new HashMap<URL, String>();       // expect: url-in-hash-collection
    private final Set<URL> seen = new HashSet<>();                           // expect: url-in-hash-collection
    private ConcurrentHashMap<java.net.URL, Integer> hits;                   // expect: url-in-hash-collection

    boolean check(URL a, URL b) {
        visit(new HashSet<URL>());                                           // expect: url-in-hash-collection
        int h = a.hashCode();                                                // expect: url-in-hash-collection
        return a.equals(b);                                                  // expect: url-in-hash-collection
    }

    void visit(Set<URI> uris) { }

    // Compliant
    private final Map<URI, String> byUri = new HashMap<>();
    private final Map<String, URL> byName = new HashMap<>();
    private final List<URL> ordered = null;
    private final TreeMap<URL, String> sorted = null;

    boolean fine(URI a, URI b, URLHolder holder) {
        return a.equals(b) && a.hashCode() == b.hashCode() && holder.equals(a);
    }
}
