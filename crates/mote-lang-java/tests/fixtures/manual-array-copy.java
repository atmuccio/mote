import java.util.*;

class Copies {
    int[] values = new int[10];
    String[] names = new String[10];

    void arrays(int count, int[] copy, long[] wide, String[] targets, Object[] objs) {
        for (int i = 0; i < count; i++) {                          // expect: manual-array-copy
            copy[i] = values[i];
        }
        for (int i = 2; i <= count; ++i)                           // expect: manual-array-copy
            copy[i - 2] = values[i];
        for (int j = 0; j < count; j += 1) {                       // expect: manual-array-copy
            targets[j + 1] = this.names[j];
        }

        // Fine: same array (a shift), conversions, casts, extra work, other steps.
        for (int i = 0; i < count; i++) {
            copy[i] = copy[i + 1];
        }
        for (int i = 0; i < count; i++) {
            wide[i] = values[i];
        }
        for (int i = 0; i < count; i++) {
            targets[i] = (String) objs[i];
        }
        for (int i = 0; i < count; i++) {
            copy[i] = values[i];
            count--;
        }
        for (int i = 0; i < count; i += 2) {
            copy[i] = values[i];
        }
        for (int i = 0; i < count; i++) {
            copy[i] = values[count - i];
        }
        for (int i = 0; i < count; i++) {
            copy[values[i]] = values[i];
        }
        int k;
        for (k = 0; k < count; k++) {
            copy[k] = values[k];
        }
        copy[k] = -1;
        for (int i = 0, j = 0; i < count; i++) {
            copy[i] = values[i];
        }
    }

    void collections(List<String> source, Set<String> target, List<Integer> ints, List<Long> longs) {
        for (String s : source) {                                  // expect: manual-array-copy
            target.add(s);
        }
        for (String s : names)                                     // expect: manual-array-copy
            target.add(s);
        for (var s : source) {                                     // expect: manual-array-copy
            target.add(s);
        }

        // Fine: conversions, extra work, filtering, same collection, primitive arrays, unknown types.
        for (int v : ints) {
            ints.add(v);
        }
        for (Object o : source) {
            target.add((String) o);
        }
        for (String s : source) {
            target.add(s.trim());
        }
        for (String s : source) {
            if (!s.isEmpty()) {
                target.add(s);
            }
        }
        for (String s : source) {
            source.add(s);
        }
        for (int v : values) {
            ints.add(v);
        }
    }

    void unknown(Foo foo, Bar bar) {
        for (String s : foo.items()) {
            bar.add(s);
        }
    }
}
