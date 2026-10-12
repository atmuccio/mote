import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Collection;
import java.util.Deque;
import java.util.LinkedList;
import java.util.List;

class LinearAccessInLoop {
    private final List<String> queue = new ArrayList<>();
    private List<String> mutable = new ArrayList<>();

    void linked(LinkedList<String> names) {
        for (int i = 0; i < names.size(); i++) {
            print(names.get(i));                                   // expect: linear-access-in-loop
        }
        List<String> copy = new LinkedList<>(names);
        for (int i = 1; i < copy.size(); i++) {
            if (copy.get(i - 1).equals(copy.get(i))) {             // expect: linear-access-in-loop, linear-access-in-loop
                print("dup");
            }
        }
    }

    void arrays(List<String> input) {
        ArrayList<String> pending = new ArrayList<>(input);
        while (!pending.isEmpty()) {
            print(pending.remove(0));                              // expect: linear-access-in-loop
        }
        List<String> reversed = new ArrayList<>();
        for (String s : input) {
            reversed.add(0, s);                                    // expect: linear-access-in-loop
        }
        do {
            queue.remove(0);                                       // expect: linear-access-in-loop
        } while (!queue.isEmpty());
    }

    // Compliant
    void fine(LinkedList<String> names, List<String> unknown, ArrayList<String> list, Collection<String> c) {
        for (int i = 0; i < 10; i++) {
            print(names.get(0));                                   // constant position
            print(names.getFirst());
            unknown.remove(0);                                     // class not known
            list.remove(i);                                        // not the front
            list.add(i, "x");
        }
        for (String s : names) {
            print(s);
        }
        print(list.remove(0));                                     // not in a loop
        for (String s : list.remove(0).split(",")) {              // the iterable is evaluated once
            print(s);
        }
        for (int i = 0; i < 3; i++) {
            if (list.size() > 5) {
                list.remove(0);                                    // runs once, then leaves
                break;
            }
        }
        while (true) {
            list.add(0, "x");
            return;
        }
    }

    void reassignedOrMutable(List<String> other) {
        List<String> l = new ArrayList<>();
        l = other;                                                  // no longer known to be an ArrayList
        for (int i = 0; i < 3; i++) {
            l.remove(0);
            mutable.remove(0);                                      // non-final field
        }
        Deque<String> d = new ArrayDeque<>(other);
        while (!d.isEmpty()) {
            d.removeFirst();
        }
    }

    void lambdas(ArrayList<String> list, LinkedList<String> linked) {
        for (int i = 0; i < 3; i++) {
            Runnable r = () -> list.remove(0);                     // runs in another context
            final int k = i;
            Runnable q = () -> print(linked.get(k));
        }
        int j = 0;
        while (j < linked.size()) {
            print(linked.get(j));                                   // not a for counter
            j++;
        }
    }

    static void print(String s) {
    }
}

class LinearAccessPerPass {
    void freshListEachPass(List<List<String>> groups, String header) {
        for (List<String> g : groups) {
            List<String> withHeader = new ArrayList<>(g);
            withHeader.add(0, header);                             // a new list on every pass
            LinearAccessInLoop.print(withHeader.remove(0));
        }
        for (List<String> g : groups) {
            List<String> rows = new ArrayList<>(g);
            for (String s : g) {
                rows.add(0, s);                                    // expect: linear-access-in-loop
            }
        }
    }
}
