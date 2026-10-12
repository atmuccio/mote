import java.security.Provider;
import java.util.List;
import java.util.Map;
import java.util.jar.Attributes;

class Lookups {
    List<String> names(boolean found, List<String> list) {
        return found ? list : null;                                    // expect: collection-return-null
    }

    Map<String, Integer> sizes(Map<String, Integer> m) {
        return m != null ? m : (null);                                 // expect: collection-return-null
    }

    int[] codes(int[] c, boolean a, boolean b) {
        return a ? c : b ? new int[0] : null;                          // expect: collection-return-null
    }

    Attributes attributes(boolean ok) {
        if (!ok) {
            return null;                                               // expect: collection-return-null
        }
        return new Attributes();
    }

    Provider provider() {
        return null;                                                   // a security provider: not a collection
    }

    String label(boolean found) {
        return found ? "x" : null;                                     // not a collection
    }

    byte[] data(boolean found, byte[] b) {
        return found ? b : null;                                       // byte[]: null means "no data"
    }

    List<String> pick(boolean found, List<String> list) {
        return found ? list : List.of();
    }

    Object any(boolean found) {
        return found ? List.of() : null;                               // declared Object
    }
}
