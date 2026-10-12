package com.example.orders;

import java.math.BigDecimal;
import java.time.LocalDate;
import java.util.List;
import java.util.Objects;
import java.util.Optional;
import java.util.UUID;
import java.util.concurrent.TimeUnit;
import com.example.shared.Money;
import com.example.shared.Tagged;

public class OrderChecks {

    enum Status { NEW, ACTIVE, CLOSED }

    enum Kind implements Tagged { A, B }

    record Point(int x, int y) {}

    record Custom(String v) {
        @Override
        public boolean equals(Object o) {
            return o instanceof String s ? s.equals(v) : o instanceof Custom c && c.v.equals(v);
        }
    }

    static final class Plain {
        int n;
    }

    static class Open {
        int n;
    }

    static final class WithEquals {
        @Override
        public boolean equals(Object o) {
            return o instanceof String;
        }
    }

    interface Shape {}

    private Long id;
    private Status status;
    private Kind kind;

    boolean positives(Integer count, String name, StringBuilder sb, UUID uuid,
                      Optional<String> maybe, LocalDate day, BigDecimal amount, TimeUnit unit,
                      Point p, Plain plain, Money money, Shape shape) {
        boolean b = id.equals(42);                          // expect: equals-unrelated-types
        b |= status.equals("ACTIVE");                       // expect: equals-unrelated-types
        b |= "ACTIVE".equals(status);                       // expect: equals-unrelated-types
        b |= name.equals(sb);                               // expect: equals-unrelated-types
        b |= sb.equals(name);                               // expect: equals-unrelated-types
        b |= count.equals(id);                              // expect: equals-unrelated-types
        b |= uuid.equals(name);                             // expect: equals-unrelated-types
        b |= maybe.equals(name);                            // expect: equals-unrelated-types
        b |= day.equals(name);                              // expect: equals-unrelated-types
        b |= amount.equals(5L);                             // expect: equals-unrelated-types
        b |= unit.equals("SECONDS");                        // expect: equals-unrelated-types
        b |= kind.equals(Status.NEW);                       // expect: equals-unrelated-types
        b |= kind.equals("A");                              // expect: equals-unrelated-types
        b |= p.equals(name);                                // expect: equals-unrelated-types
        b |= plain.equals(count);                           // expect: equals-unrelated-types
        b |= plain.equals(shape);                           // expect: equals-unrelated-types
        b |= name.equals(money);                            // expect: equals-unrelated-types
        b |= Objects.equals(id, 7);                         // expect: equals-unrelated-types
        b |= name.equals('c');                              // expect: equals-unrelated-types
        return b;
    }

    boolean negatives(Integer count, String name, Object any, CharSequence cs, Number num,
                      Comparable<String> cmp, Money money, Custom custom, WithEquals we,
                      Open open, Shape shape, Plain plain, List<String> list, BigDecimal amount,
                      Tagged tag, Point p) {
        boolean b = id.equals(42L);
        b |= id.equals(null);
        b |= count.equals(3);
        b |= name.equals(any);
        b |= name.equals(cs);
        b |= count.equals(num);
        b |= name.equals(cmp);
        b |= status.equals(Status.NEW);
        b |= money.equals(name);              // unknown receiver type: its equals may accept anything
        b |= custom.equals(name);             // record with its own equals
        b |= we.equals(name);                 // final class with its own equals
        b |= open.equals(name);               // subclasses in other files may override equals
        b |= shape.equals(plain);             // interface receiver: not reported
        b |= list.equals(name);               // interface receiver: not reported
        b |= amount.equals(money);            // Money might extend BigDecimal
        b |= kind.equals(tag);                // Kind implements Tagged
        b |= p.equals(new Point(1, 2));
        b |= Objects.equals(name, any);
        b |= Objects.equals(null, name);
        b |= this.equals(name);
        b |= eq(name, count);
        return b;
    }

    <T> boolean generic(T value, String name) {
        return name.equals(value);
    }

    boolean lambdas(List<Long> ids) {
        return ids.stream().anyMatch(x -> x.equals(1));   // lambda parameter type unknown
    }

    static boolean eq(Object a, Object b) {
        return a.equals(b);
    }
}

class ShadowsObjects {
    static class Objects {
        static boolean equals(Object a, Object b) {
            return true;
        }
    }

    boolean check(Long id) {
        return Objects.equals(id, 7);
    }
}
