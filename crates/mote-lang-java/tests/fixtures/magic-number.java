import java.util.*;

class Magic {
    static final int MAX = 42;
    private static final double RATE = 0.075;
    static final int[] TABLE = {3, 5, 7};
    final int instanceLimit = 99;                            // expect: magic-number
    int counter = 17;                                        // expect: magic-number

    void m(int[] a) {
        int ok = 0 + 1 + 2 - 1;
        int neg = -1;
        long big = 1L;
        double d = 1.0 + 2.0 + 0.0;
        int x = a[3];                                        // expect: magic-number
        if (a.length > 100) {                                // expect: magic-number
            sleep(86_400);                                   // expect: magic-number
        }
        double t = 0.5 * x;                                  // expect: magic-number
        int n = -7;                                          // expect: magic-number
        int hex = 0xFF;                                      // expect: magic-number
        int two = 0x2;
    }

    @SuppressWarnings("x")
    @Timeout(value = 30)
    void annotated() {}

    @Override
    public int hashCode() {
        return 31 * 17 + 7;
    }

    int hashCode(int seed) {
        return seed * 31;                                    // expect: magic-number
    }

    void sleep(int s) {}

    interface Api {
        int TIMEOUT = 5000;
        int size() ;
    }

    enum Planet {
        MERCURY(3.303e+23, 2.4397e6),
        EARTH(5.976e+24, 6.37814e6) {
            double gravity() { return 9.81; }                // expect: magic-number
        };
        Planet(double mass, double radius) {}
    }

    @interface Timeout {
        int value() default 60;
    }
}
