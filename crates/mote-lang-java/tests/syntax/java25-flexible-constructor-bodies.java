// Java 25: flexible constructor bodies (JEP 513): statements before `super(...)` and `this(...)`.
import java.util.Objects;

class Account extends Ledger {
    private final long limit;

    Account(long limit) {
        if (limit < 0) {
            throw new IllegalArgumentException("limit");
        }
        this.limit = limit;
        super(Math.min(limit, 1_000));
        open();
    }

    Account(String limit) {
        Objects.requireNonNull(limit);
        long parsed = Long.parseLong(limit.strip());
        this(parsed);
    }

    Account() {
        this(0L);
    }

    private void open() {}
}

class Ledger {
    Ledger(long size) {}
}

record Range(int low, int high) {
    Range(int high) {
        int low = Math.min(0, high);
        this(low, high);
    }
}
