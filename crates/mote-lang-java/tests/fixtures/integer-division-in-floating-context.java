import java.util.List;
import java.util.function.DoubleSupplier;

class Division {
    private int done;
    private long total;
    private double ratio = 1 / 3;                               // expect: integer-division-in-floating-context
    private static final float HALF = 1 / 2;                    // expect: integer-division-in-floating-context

    double positives(int a, int b, long c, short s, Integer boxed) {
        double d = a / b;                                       // expect: integer-division-in-floating-context
        float f = (c / 2);                                      // expect: integer-division-in-floating-context
        ratio = done / total;                                   // expect: integer-division-in-floating-context
        ratio += s / a;                                         // expect: integer-division-in-floating-context
        double e = boxed / a;                                   // expect: integer-division-in-floating-context
        double g = (double) (a / b);                            // expect: integer-division-in-floating-context
        return a / b;                                           // expect: integer-division-in-floating-context
    }

    float average(int sum, int count) {
        return sum / count;                                     // expect: integer-division-in-floating-context
    }

    double negatives(int a, int b, double x, List<Integer> list, Unknown u) {
        double d = (double) a / b;
        double e = a / 2.0;
        double f = x / a;
        int g = a / b;
        long h = a / b;
        double i = u.count() / a;                               // unknown type
        double j = a / b * 2.0;                                 // nested in larger arithmetic: not reported
        DoubleSupplier k = () -> { return a / b; };             // lambda: return type unknown here
        double m = list.stream().mapToDouble(v -> v / 2).sum(); // lambda parameter
        Object o = new Object() {
            int half(int n) { return n / 2; }
        };
        return d + e + f + g + h + i + j + m;
    }

    int intMethod(int a, int b) {
        return a / b;
    }
}
