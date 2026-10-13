// Java 25: a no-argument constructor with statements before `super()` does work of its own.
public class Ledger {
    public Ledger() {
        audit();
        super();
    }

    static void audit() {}
}

public class Journal {                                              // expect-next-line: redundant-default-constructor
    public Journal() {
        super();
    }
}
