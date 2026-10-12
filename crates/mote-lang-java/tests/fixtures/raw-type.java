import java.util.ArrayList;
import java.util.Collections;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.Map.Entry;

class Legacy {
    private List names;                                               // expect: raw-type
    private Map<String, List> byCity = new HashMap<>();               // expect: raw-type
    private List<String> typed = new ArrayList<>();
    private Class type;                                               // expect: raw-type
    private Class<?> anyType;

    List load(Map source) {                                           // expect: raw-type raw-type
        List out = new ArrayList();                                   // expect: raw-type raw-type
        for (Object o : source.entrySet()) {
            Entry e = (Entry) o;                                      // expect: raw-type
            Map.Entry<String, String> ok = null;
        }
        if (source instanceof HashMap) {
            out.add(List.class);
        }
        List[] buckets = new List[4];                                 // arrays: accepted
        Object cast = (List) source.get("x");                         // casts: accepted
        return Collections.EMPTY_LIST;                                // expect: raw-type
    }

    @Override
    public boolean equals(Object other) {
        return false;
    }

    @SuppressWarnings("rawtypes")
    void suppressed() {
        List quiet = new ArrayList();
    }

    @SuppressWarnings({"unchecked", "rawtypes"})
    private Map quietField;

    static class Impl implements Comparable {
        @Override
        public int compareTo(Object o) {
            return 0;
        }
    }

    interface Callback {
        void on(List items);                                          // expect: raw-type
    }

    static class CallbackImpl implements Callback {
        @Override
        public void on(List items) {                                  // dictated by the interface
        }
    }

    static class List2 {
    }

    void own(List2 mine, java.util.function.Function<String, Integer> f) {
        Runnable r = () -> {};
    }
}

class Captors {
    void capture() {
        Captor<List> captor = Captor.forClass(List.class);                   // class token: accepted
        Captor<List> other = Captor.create();                                // expect: raw-type
    }
}
