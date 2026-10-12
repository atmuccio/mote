import java.io.Serializable;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.HashSet;
import java.util.LinkedList;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;
import java.util.Vector;

class Orders {
    private ArrayList<String> names = new ArrayList<>();            // expect: concrete-collection-type
    protected HashMap<String, Integer> totals;                      // expect: concrete-collection-type
    java.util.HashSet<Long> ids;                                    // expect: concrete-collection-type
    private LinkedList<String> queue = new LinkedList<>();          // addFirst is used: keep
    private TreeMap<String, String> sorted = new TreeMap<>();       // firstKey is used: keep
    private Vector<String> legacy;                                  // elementAt is used: keep
    private List<String> fine = new ArrayList<>();
    @Deprecated
    private HashMap<String, String> annotated;

    public ArrayList<String> names() {                              // expect: concrete-collection-type
        return names;
    }

    void process(HashMap<String, Integer> input, List<String> ok) { // expect: concrete-collection-type
        ArrayList<String> local = new ArrayList<>();                // locals are not reported
        local.add("x");
        input.put("a", 1);
    }

    void grow(ArrayList<String> buffer) {
        buffer.ensureCapacity(100);                                 // implementation-specific call: keep
    }

    Orders(HashSet<String> seed) {                                  // expect: concrete-collection-type
    }

    void use() {
        queue.addFirst("x");
        String k = sorted.firstKey();
        String e = legacy.elementAt(0);
    }

    Map<String, List<String>> nested() {
        return new HashMap<>();
    }

    ArrayList<String>[] arrays() {                                  // arrays of collections: not reported
        return null;
    }
}

class Child extends Base {
    @Override
    public ArrayList<String> items(HashMap<String, String> filter) { // @Override: signature is dictated
        return null;
    }

    void anonymous() {
        Object o = new Handler() {
            public void handle(ArrayList<String> rows) { }          // anonymous class method
        };
    }
}

class Dto implements Serializable {
    private ArrayList<String> tags;                                 // Serializable class: deliberate
}

class Custom {
    private ArrayList names;                                        // expect: concrete-collection-type
    private com.acme.ArrayList<String> other;                       // not java.util
}
