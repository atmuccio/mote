import java.util.ArrayList;
import java.util.List;

class Loops {
    private int[] values;

    void positives(String[] names, List<String> list, ArrayList<Integer> nums) {
        for (int i = 0; i < names.length; i++) {                    // expect: index-loop-could-be-foreach
            print(names[i]);
        }
        for (int i = 0; i < list.size(); i++) {                     // expect: index-loop-could-be-foreach
            String s = list.get(i);
            print(s + list.get(i));
        }
        for (int j = 0; j < nums.size(); ++j) {                     // expect: index-loop-could-be-foreach
            print(nums.get(j));
        }
        for (int k = 0; k < names.length; k += 1) print(names[k]);  // expect: index-loop-could-be-foreach
    }

    void negatives(String[] names, List<String> list, NodeList nodes, Unknown u) {
        for (int i = 0; i < names.length; i++) {
            names[i] = "";                                          // write
        }
        for (int i = 0; i < names.length; i++) {
            print(i + names[i]);                                    // index used on its own
        }
        for (int i = 1; i < names.length; i++) {                    // starts at 1
            print(names[i]);
        }
        for (int i = 0; i <= names.length - 1; i++) {
            print(names[i]);
        }
        for (int i = 0; i < names.length; i += 2) {
            print(names[i]);
        }
        for (int i = 0; i < list.size(); i++) {
            print(list.get(i));
            list.remove(0);                                         // the list is modified
        }
        for (int i = 0; i < names.length; i++) {
            print(names[i] + names[i + 1]);                         // neighbour access
        }
        for (int i = 0; i < u.size(); i++) {                        // unknown type
            print(u.get(i));
        }
        for (int i = 0; i < names.length; i++) {
            print(other(i));
        }
        for (int i = 0, n = names.length; i < n; i++) {
            print(names[i]);
        }
        for (int i = 0; i < this.values.length; i++) {              // qualified target: not handled
            print(this.values[i]);
        }
        for (int i = 0; i < names.length; i++) {
            names[i]++;
        }
        for (int i = 0; i < names.length; i++) {
            print(list.get(i));                                     // other collection
        }
    }

    void print(Object o) { }

    String other(int i) { return ""; }
}
