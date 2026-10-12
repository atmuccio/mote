// mote-test: params java_release=17
import java.math.BigDecimal;
import java.math.BigInteger;

class Values {
    private static final String NOT_FOUND = new String("__missing__");

    Object positives(String text) {
        String a = new String("fixed");                             // expect: boxed-primitive-constructor
        String b = new String();                                    // expect: boxed-primitive-constructor
        BigDecimal zero = new BigDecimal(0);                        // expect: boxed-primitive-constructor
        BigDecimal one = new BigDecimal("1");                       // expect: boxed-primitive-constructor
        BigDecimal ten = new BigDecimal(10L);                       // expect: boxed-primitive-constructor
        BigDecimal many = new BigDecimal(250);                      // no constant for it
        BigInteger two = new BigInteger("2");                       // expect: boxed-primitive-constructor
        BigInteger big = new BigInteger("123456");
        BigDecimal twoDecimal = new BigDecimal(2);                  // BigDecimal.TWO needs Java 19
        return java.util.List.of(a, b, zero, one, ten, many, two, big, twoDecimal);
    }

    // VAT rates: whole numbers without a constant read best as they are.
    static final BigDecimal VAT_REDUCED = new BigDecimal("19");
    static final BigDecimal VAT_STANDARD = new BigDecimal("20");
    static final BigDecimal VAT_HIGH = new BigDecimal("21");

    Object negatives(String text, char[] chars, byte[] bytes) {
        String copy = new String(chars);
        String decoded = new String(bytes, java.nio.charset.StandardCharsets.UTF_8);
        String joined = new String("a" + text);
        BigDecimal scaled = new BigDecimal("1.0");                  // another scale than ONE
        BigDecimal exp = new BigDecimal("1e3");
        BigDecimal fromText = new BigDecimal(text);
        BigDecimal fromDouble = new BigDecimal(2.5);
        BigInteger radix = new BigInteger("ff", 16);
        BigInteger huge = new BigInteger("123456789012345678901234567890");
        BigInteger negative = new BigInteger("-1");
        String sentinel = lookup(text);
        if (sentinel == NOT_FOUND) {
            return null;
        }
        return java.util.List.of(copy, decoded, joined, scaled, exp, fromText, fromDouble, radix, huge, negative);
    }

    String lookup(String k) {
        return k;
    }

    static class String2 {
    }
}
