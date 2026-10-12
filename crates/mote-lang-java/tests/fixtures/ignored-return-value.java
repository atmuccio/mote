import java.math.BigDecimal;
import java.math.BigInteger;
import java.math.RoundingMode;
import java.time.Duration;
import java.time.LocalDate;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.stream.IntStream;
import java.util.stream.Stream;

class Invoice {
    private String name = " x ";
    private BigDecimal total = BigDecimal.ZERO;

    void positives(String s, BigDecimal tax, BigInteger big, LocalDate date, Duration d,
                   Optional<String> opt, Stream<String> stream, IntStream ints, List<String> list) {
        s.trim();                                                   // expect: ignored-return-value
        name.toUpperCase();                                         // expect: ignored-return-value
        this.name.replace("a", "b");                                // expect: ignored-return-value
        s.substring(1);                                             // expect: ignored-return-value
        "literal".concat(s);                                        // expect: ignored-return-value
        String.format("%s", s);                                     // expect: ignored-return-value
        total.add(tax);                                             // expect: ignored-return-value
        total.setScale(2, RoundingMode.HALF_UP);                    // expect: ignored-return-value
        big.shiftLeft(2);                                           // expect: ignored-return-value
        date.plusDays(1);                                           // expect: ignored-return-value
        date.withDayOfMonth(1);                                     // expect: ignored-return-value
        d.multipliedBy(2);                                          // expect: ignored-return-value
        opt.orElse("none");                                         // expect: ignored-return-value
        opt.map(v -> v + "!");                                      // expect: ignored-return-value
        stream.filter(v -> !v.isEmpty());                           // expect: ignored-return-value
        list.stream().map(v -> v.length());                         // expect: ignored-return-value
        ints.boxed();                                               // expect: ignored-return-value
    }

    void negatives(String s, StringBuilder sb, Map<String, String> map, List<BigDecimal> amounts,
                   Optional<String> opt, Stream<String> stream, Builder builder, Unknown u) {
        s = s.trim();
        String t = name.toUpperCase();
        sb.replace(0, 1, "x");                                      // StringBuilder mutates
        sb.append("x");
        map.replace("k", "v");                                      // Map.replace mutates
        amounts.add(BigDecimal.ONE);                                // List.add mutates
        total = total.add(BigDecimal.TEN);
        opt.orElseThrow(IllegalStateException::new);               // called for the exception
        opt.orElseGet(this::fallback);
        opt.ifPresent(System.out::println);
        opt.map(v -> { System.out.println(v); return v; });         // side effects in the function
        opt.map(this::save);                                        // method reference: may have effects
        stream.forEach(System.out::println);                        // terminal operation
        builder.withName("x");                                      // unknown builder type
        u.trim();                                                   // unknown type
        s.intern();
        trim();
        try {
            total.setScale(2);                                      // may be a validation by exception
        } catch (ArithmeticException e) {
            s.trim();                                               // expect: ignored-return-value
        }
    }

    String fallback() { return ""; }

    String save(String v) { return v; }

    void trim() { }
}
