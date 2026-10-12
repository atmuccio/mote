import java.util.ArrayList;
import java.util.List;
import java.util.Set;

class Arrays2 {
    private final List<String> names = new ArrayList<>();

    void m(List<String> list, Set<Integer> ids, Unknown u, Holder h) {
        String[] a = list.toArray(new String[list.size()]);         // expect: presized-to-array
        Integer[] b = ids.toArray(new Integer[ids.size()]);         // expect: presized-to-array
        String[] c = names.toArray(new String[names.size()]);       // expect: presized-to-array
        String[] d = this.names.toArray(new String[this.names.size()]); // expect: presized-to-array
        Object[] e = u.toArray(new Object[u.size()]);               // expect: presized-to-array

        String[] ok1 = list.toArray(new String[0]);
        String[] ok2 = list.toArray(String[]::new);
        String[] ok3 = list.toArray(new String[names.size()]);      // sized from another collection
        String[] ok4 = list.toArray(new String[list.size() + 1]);
        Object[] ok5 = list.toArray();
        String[] ok6 = h.toArray(new String[h.size()]);             // Holder is declared here, not a Collection
    }
}

class Holder {
    int size() { return 0; }
    String[] toArray(String[] a) { return a; }
}
