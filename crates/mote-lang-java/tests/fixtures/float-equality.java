import java.util.List;

class Floats {
    private double total;
    private float scale;
    private static final double EPS = 1e-9;

    boolean positives(double a, double b, float f, int n, Double boxed) {
        boolean r1 = a == b;                         // expect: float-equality
        boolean r2 = total != a * 2;                 // expect: float-equality
        boolean r3 = f == scale;                     // expect: float-equality
        boolean r4 = n == a;                         // expect: float-equality
        boolean r5 = a == 0.1;                       // expect: float-equality
        boolean r6 = boxed == a;                     // expect: float-equality
        boolean r7 = Math.sqrt(a) == b;              // expect: float-equality
        boolean r8 = a == Double.NaN;                // expect: float-equality
        return r1 && r2 && r3 && r4 && r5 && r6 && r7 && r8;
    }

    boolean negatives(double a, double b, int n, int m, Double x, Double y, Unknown u, List<Double> list) {
        boolean n1 = a != a;                         // NaN test
        boolean n2 = a == 0;                         // exact zero guard
        boolean n3 = a == 0.0;
        boolean n4 = b == -1;                        // sentinel
        boolean n5 = a == 1.0d;
        boolean n6 = n == m;
        boolean n7 = x == y;                         // reference comparison: another problem
        boolean n8 = u.value() == a;                 // unknown type
        boolean n9 = a == Double.MAX_VALUE;
        boolean n10 = b != Double.NEGATIVE_INFINITY;
        boolean n11 = Math.abs(a - b) < EPS;
        boolean n12 = list.stream().anyMatch(v -> v == a);   // lambda parameter type is unknown
        boolean n13 = Double.compare(a, b) == 0;
        boolean n14 = a == Math.floor(a);            // whole-number test
        boolean n15 = (long) b != b;
        return n1 && n2 && n3 && n4 && n5 && n6 && n7 && n8 && n9 && n10 && n11 && n12 && n13 && n14 && n15;
    }
}
