import java.util.ArrayList;
import java.util.Iterator;
import java.util.List;
import java.util.Map;
import java.util.Set;

class Tables {
    private final List<String> columns = new ArrayList<>();
    private Map<String, Integer> sizes;

    void positives(Set<String> keys) {
        for (Iterator<String> it = columns.iterator(); it.hasNext();) {     // expect: index-loop-could-be-foreach
            String c = it.next();
            print(c);
        }
        for (Iterator it = keys.iterator(); it.hasNext();) {                // expect: index-loop-could-be-foreach
            String key = (String) it.next();
            print(key);
        }
        for (Iterator<Map.Entry<String, Integer>> i = sizes.entrySet().iterator(); i.hasNext(); ) { // expect: index-loop-could-be-foreach
            print(i.next().getKey());
        }
        for (var it = columns.iterator(); it.hasNext();) {                  // expect: index-loop-could-be-foreach
            log("next");
            print(it.next());
        }
    }

    void negatives(Iterable<String> any, Other other, List<String> list, String[] names) {
        for (Iterator<String> it = columns.iterator(); it.hasNext();) {
            if (it.next().isEmpty()) {
                it.remove();                                                // needs the iterator
            }
        }
        for (Iterator<String> it = columns.iterator(); it.hasNext();) {
            String a = it.next();
            String b = it.next();                                           // two elements per pass
            print(a + b);
        }
        for (Iterator<String> it = columns.iterator(); it.hasNext();) {
            if (list.isEmpty()) {
                print(it.next());                                           // only on some passes
            }
        }
        for (Iterator<String> it = columns.iterator(); it.hasNext() && list.isEmpty();) {
            print(it.next());                                               // extra condition
        }
        for (Iterator<String> it = columns.iterator(); it.hasNext(); count++) {
            print(it.next());                                               // extra update
        }
        for (Iterator<String> it = columns.iterator(); it.hasNext();) {
            boolean ok = list.isEmpty() && it.next() != null;               // short-circuit
        }
        for (Iterator<String> it = columns.iterator(); it.hasNext();) {
            String s = it.next();
            helper(it);                                                     // iterator passed on
        }
        for (Iterator<String> it = other.iterator(); it.hasNext();) {
            print(it.next());                                               // unknown type
        }
        for (Iterator<String> it = list.listIterator(); it.hasNext();) {
            print(it.next());                                               // not iterator()
        }
        for (Iterator<String> it = columns.iterator(); it.hasNext();) {
            if (list.isEmpty()) {
                break;
            }
            print(it.next());                                               // a pass may end before next()
        }
        for (Iterator<String> it = columns.iterator(); it.hasNext();) {
            Runnable r = () -> print(it.next());                            // in a lambda
        }
        for (Iterator<String> it = columns.iterator(); it.hasNext();)
            print(it.next());                                               // not a block: left alone
    }

    int count;

    void print(Object o) { }
    void log(String s) { }
    void helper(Iterator<String> it) { }
}
