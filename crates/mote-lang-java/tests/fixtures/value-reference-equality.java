import java.math.BigDecimal;
import java.math.BigInteger;
import java.time.LocalDate;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.UUID;

class Quantities {
    private Integer reserved;
    private static final Integer LIMIT = 1000;

    boolean boxed(Integer a, Integer b, Long big, Long other, Character c, Character d,
                  Map<String, Integer> stock, List<Double> prices) {
        boolean r1 = a == b;                                    // expect: value-reference-equality
        boolean r2 = big != other;                              // expect: value-reference-equality
        boolean r3 = c == d;                                    // expect: value-reference-equality
        boolean r4 = stock.get("apple") == reserved;            // expect: value-reference-equality
        boolean r5 = Integer.valueOf(7) == this.reserved;       // expect: value-reference-equality
        boolean r6 = prices.get(0) == prices.get(1);            // expect: value-reference-equality
        boolean r7 = (a) == (LIMIT);                            // expect: value-reference-equality
        for (Integer q : stock.values()) {
            if (q == a) {                                       // expect: value-reference-equality
                return true;
            }
        }
        return r1 && r2 && r3 && r4 && r5 && r6 && r7;
    }

    boolean values(BigDecimal price, BigDecimal limit, BigInteger x, BigInteger y,
                   LocalDate from, LocalDate to, UUID id, UUID other,
                   Optional<String> name, Optional<String> alias) {
        boolean v1 = price == BigDecimal.ZERO;                  // expect: value-reference-equality
        boolean v2 = price != limit;                            // expect: value-reference-equality
        boolean v3 = x == y;                                    // expect: value-reference-equality
        boolean v4 = from == to;                                // expect: value-reference-equality
        boolean v5 = id != other;                               // expect: value-reference-equality
        boolean v6 = name == alias;                             // expect: value-reference-equality
        boolean v7 = LocalDate.now() == from;                   // expect: value-reference-equality
        return v1 && v2 && v3 && v4 && v5 && v6 && v7;
    }

    boolean negatives(Integer a, int n, long m, Object o, Boolean flag, Boolean other,
                      String s, String t, Class<?> k1, Class<?> k2, Color c1, Color c2,
                      Number num, Widget w, Widget w2, BigDecimal price) {
        boolean n1 = a == null;                                 // null check
        boolean n2 = null != a;
        boolean n3 = a == n;                                    // unboxed: numeric comparison
        boolean n4 = m == a;
        boolean n5 = a == 42;                                   // literal: unboxed
        boolean n6 = o == a;                                    // Object: identity on purpose
        boolean n7 = flag == other;                             // Boolean: two shared instances
        boolean n8 = flag == Boolean.TRUE;
        boolean n9 = s == t;                                    // strings have their own rule
        boolean n10 = k1 == k2;                                 // Class: identity is equality
        boolean n11 = c1 == c2;                                 // enum constants
        boolean n12 = num == a;                                 // Number vs Integer: different types
        boolean n13 = w.count() == w2.count();                  // unknown types
        boolean n14 = a.intValue() == n;
        boolean n15 = price.compareTo(BigDecimal.ONE) == 0;
        boolean n16 = a.equals(n);
        boolean n17 = Optional.empty() == Optional.<String>empty();
        Optional<String> maybe = Optional.empty();
        boolean n18 = maybe == Optional.empty();
        return n1 && n2 && n3 && n4 && n5 && n6 && n7 && n8 && n9 && n10 && n11 && n12
                && n13 && n14 && n15 && n16 && n17 && n18;
    }

    <T> boolean generic(T x, T y, List<Integer> list) {
        list.forEach(v -> check(v == reserved));               // lambda parameter: type not inferred
        return x == y;                                          // type variables: unknown
    }

    void check(boolean b) { }

    enum Color { RED, GREEN }
}
