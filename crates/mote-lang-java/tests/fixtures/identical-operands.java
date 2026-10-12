import java.util.List;
import java.util.Objects;

class Operands {
    private int width;
    private long count;
    private double ratio;
    private float scale;
    private Integer boxed;
    private String name;
    private static final int MASK = 0xff;

    boolean positives(int a, int b, boolean flag, Operands other, int[] arr) {
        boolean r1 = a == a;                                // expect: identical-operands
        boolean r2 = this.width != this.width;              // expect: identical-operands
        boolean r3 = a < a || b >= b;                       // expect: identical-operands, identical-operands
        boolean r4 = flag && flag;                          // expect: identical-operands
        boolean r5 = flag || (flag);                        // expect: identical-operands
        long d = count - count;                             // expect: identical-operands
        int q = width / width;                              // expect: identical-operands
        int m = arr[0] % arr[0];                            // expect: identical-operands
        int x = MASK ^ MASK;                                // expect: identical-operands
        int y = (a & MASK) | (a & MASK);                    // expect: identical-operands
        boolean r6 = name.equals(name);                     // expect: identical-operands
        boolean r7 = other.width == other . width;          // expect: identical-operands
        boolean r8 = boxed == boxed;                        // expect: identical-operands
        return r1 && r2 && r3 && r4 && r5 && r6 && r7 && r8 && d + q + m + x + y > 0;
    }

    boolean negatives(int a, int b, double v, float f, Operands other, List<String> list, Unknown u) {
        boolean n1 = v != v;                                // NaN test
        boolean n2 = f == f;                                // NaN test
        boolean n3 = ratio == ratio;                        // NaN test on a double field
        double n4 = v - v;                                  // floating arithmetic: skipped
        boolean n5 = a == b;
        boolean n6 = next() == next();                      // calls may differ
        boolean n7 = list.get(0) == list.get(0);
        boolean n8 = u.x == u.x;                            // unknown type: may be a double
        boolean n9 = 1 == 1;                                // two literals
        int n10 = a++ - a++;
        boolean n11 = "a b".equals("ab");
        boolean n12 = name.equals(other.name);
        boolean n13 = Objects.equals(a, a);
        int n14 = a + a;                                    // doubling is fine
        int n15 = a * a;                                    // squaring is fine
        int n16 = a << a;
        boolean n17 = new Object() == new Object();
        boolean n18 = list.stream().anyMatch(s -> s == s);  // lambda parameter type is unknown
        return n1 && n2 && n3 && n4 > 0 && n5 && n6 && n7 && n8 && n9 && n10 > 0 && n11 && n12 && n13 && n14 + n15 + n16 > 0 && n17 && n18;
    }

    int next() { return 1; }
}
