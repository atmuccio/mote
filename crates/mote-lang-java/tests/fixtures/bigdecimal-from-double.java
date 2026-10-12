import java.math.BigDecimal;
import java.math.MathContext;
import java.util.List;

class Money {
    private double rate;
    private float factor;

    List<BigDecimal> positives(double price, int quantity, Double boxed) {
        BigDecimal a = new BigDecimal(0.1);                         // expect: bigdecimal-from-double
        BigDecimal b = new BigDecimal(price);                       // expect: bigdecimal-from-double
        BigDecimal c = new BigDecimal(price * quantity);            // expect: bigdecimal-from-double
        BigDecimal d = new BigDecimal(rate, MathContext.DECIMAL64); // expect: bigdecimal-from-double
        BigDecimal e = new BigDecimal(factor);                      // expect: bigdecimal-from-double
        BigDecimal f = new BigDecimal(-0.07);                       // expect: bigdecimal-from-double
        BigDecimal g = new BigDecimal(boxed);                       // expect: bigdecimal-from-double
        BigDecimal h = new BigDecimal(1.1e-3d);                     // expect: bigdecimal-from-double
        BigDecimal i = new BigDecimal(0.1f);                        // expect: bigdecimal-from-double
        return List.of(a, b, c, d, e, f, g, h, i);
    }

    List<BigDecimal> negatives(double price, long cents, Unknown u) {
        BigDecimal a = new BigDecimal("0.1");
        BigDecimal b = BigDecimal.valueOf(price);
        BigDecimal c = new BigDecimal(cents);
        BigDecimal d = new BigDecimal(1.0);                         // exactly representable
        BigDecimal e = new BigDecimal(0.5);
        BigDecimal f = new BigDecimal(100d);
        BigDecimal g = new BigDecimal(u.amount());                  // unknown type
        BigDecimal h = new BigDecimal(42);
        BigDecimal i = new BigDecimal(-2.25);
        BigDecimal j = new BigDecimal(String.valueOf(price));
        return List.of(a, b, c, d, e, f, g, h, i, j);
    }

    // The exact binary value of a decimal is wanted on purpose (what a float makes of it).
    List<BigDecimal> binaryRoundTrip(BigDecimal value, Number n) {
        BigDecimal a = new BigDecimal(value.floatValue());
        BigDecimal b = new BigDecimal(value.doubleValue(), MathContext.DECIMAL64);
        BigDecimal c = new BigDecimal(n.doubleValue());             // expect: bigdecimal-from-double
        return List.of(a, b, c);
    }

    static class Local {
        static class BigDecimal {
            BigDecimal(double d) {}
        }

        Object make() {
            return new BigDecimal(0.1);                             // this file's own class
        }
    }
}
