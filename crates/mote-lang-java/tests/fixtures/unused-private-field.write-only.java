package com.example.orders;

import java.io.Serializable;
import java.util.List;
import javax.annotation.Nullable;
import javax.persistence.Column;
import javax.persistence.Entity;
import javax.persistence.Id;

class OrderService {
    private final Clock clock;                              // expect: unused-private-field
    private final Repository repository;
    private List<String> cache;                             // expect: unused-private-field
    private int attempts;                                   // expect: unused-private-field
    private long sum;                                       // expect: unused-private-field
    private int readAndWritten;
    private int incrementedAndRead;
    private String shadowed;                                // expect: unused-private-field
    private String param;
    private int[] slots = new int[4];
    @Nullable private String note;                          // expect: unused-private-field
    @Deprecated private int legacy;                         // expect: unused-private-field
    @SuppressWarnings("unused") private int kept;
    @SuppressWarnings("NullAway.Init") private String late; // expect: unused-private-field
    private static String orders;                           // expect: unused-private-field

    OrderService(Clock clock, Repository repository, String shadowed) {
        this.clock = clock;
        this.repository = repository;
        this.shadowed = shadowed;
    }

    void refresh() {
        cache = repository.load();
        attempts++;
        this.sum += 5;
        readAndWritten = readAndWritten + 1;
        incrementedAndRead++;
        slots[0] = 1;
        note = null;
        legacy = 2;
        late = "x";
        param = "p";
        kept = 1;
        this.orders = "o";
    }

    int report() {
        use(param);
        return incrementedAndRead;
    }

    void use(Object o) { }
}

@Entity
class Invoice {
    @Id private Long id;
    private String number;                                  // an entity: read by the persistence layer

    Invoice(String number) {
        this.number = number;
    }
}

class Mapped {
    @Column(name = "total") private long total;
    private String label;                                   // a mapped field is present: stay quiet

    void set(String l) { this.label = l; }
}

class Payload implements Serializable {
    private String body;                                    // serializable: its fields matter

    Payload(String body) {
        this.body = body;
    }
}

class LocalReads {
    private int count;                                      // same-named local is read: stays quiet

    void inc() {
        count = 1;
        int count = 2;
        System.out.println(count);
    }
}

class Holder {
    private Object keep;                                    // expect: unused-private-field

    Holder(Object o) {
        keep = o;
    }
}

class Lambdas {
    private int seen;
    private int ticks;                                      // a same-named lambda parameter is read

    java.util.function.IntSupplier supplier() {
        ticks++;
        return () -> seen;
    }

    java.util.function.IntUnaryOperator op() {
        return ticks -> ticks + 1;
    }
}

class LambdaParams {
    private int calls;                                      // expect: unused-private-field

    java.util.function.IntUnaryOperator op() {
        calls++;
        return calls -> 0;
    }
}
