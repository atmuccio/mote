import java.util.HashMap;
import java.util.List;
import java.util.Map;

class Sizes {
    private static final String WIDTH = "width";
    private final Map<String, Integer> size = new HashMap<>();
    private String name;
    private volatile int state;
    private int[] cells = new int[4];

    void positives(int w, int h, String key, Map<String, Object> props) {
        size.put("width", w);                                       // expect: overwritten-before-read
        size.put("width", h);
        size.put(WIDTH, w);                                         // expect: overwritten-before-read
        size.put(WIDTH, h);
        props.put(key, "a");                                        // expect: overwritten-before-read
        props.put(key, "b");
        cells[0] = w;                                               // expect: overwritten-before-read
        cells[0] = h;
        this.name = "a";                                            // expect: overwritten-before-read
        this.name = key;
        int local = 0;
        local = 1;                                                  // expect: overwritten-before-read
        local = w + h;
        switch (w) {
            case 1:
                name = "x";                                         // expect: overwritten-before-read
                name = "y";
                break;
            default:
                break;
        }
        consume(local);
    }

    Sizes(String first, String second) {
        this.name = first;                                          // expect: overwritten-before-read
        this.name = second;
    }

    void negatives(int w, int h, Registry registry, List<String> list, int i) {
        size.put("width", w);
        size.put("height", h);
        size.put("width", size.get("width") + 1);                   // reads the previous value
        registry.put("k", w);                                       // unknown type
        registry.put("k", h);
        cells[i] = w;
        cells[i + 1] = h;
        cells[i] = w;
        cells[i] = cells[i] * 2;                                    // reads the element
        this.name = "a";
        this.name = describe();                                     // a method of this object may read it
        state = 1;                                                  // volatile: the intermediate value may be observed
        state = 2;
        int x = 0;
        x += 1;
        x = 5;
        String line = next();                                       // declaration, not an assignment
        line = next();
        line = next();                                              // first value has side effects
        list.add("a");
        list.add("a");
        try {
            line = null;
            line = next();                                          // `null` stays visible if next() throws
        } finally {
            consume(line);
        }
        consume(x);
        consume(line);
    }

    String describe() { return name; }

    String next() { return ""; }

    void consume(Object o) { }
}
