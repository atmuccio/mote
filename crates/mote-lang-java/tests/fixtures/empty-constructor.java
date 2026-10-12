import javax.inject.Inject;

public class Invoice {
    private String number;

    // expect-next-line: empty-constructor
    public Invoice() {}

    public Invoice(String number) {
        this.number = number;
    }

    // expect-next-line: empty-constructor
    protected Invoice(int ignored) {
    }

    Invoice(long id) {
        // Reserved for the persistence layer.
    }

    Invoice(char c) {} // kept for binary compatibility

    Invoice(double d) {
        this();
    }

    @Inject
    Invoice(Object context) {}
}

class Standalone {
    // expect-next-line: empty-constructor
    public Standalone() {
    }

    void work() { }
}

abstract class Base {
    // expect-next-line: empty-constructor
    protected Base() {}
}

final class Utils {
    private Utils() {}

    static int twice(int x) { return 2 * x; }
}

class Child extends Base {
    Child() {
        super();
    }
}

class Singleton {
    static final Singleton INSTANCE = new Singleton();

    private Singleton() {
    }
}

enum Level {
    LOW, HIGH;

    Level() {}
}

record Range(int from, int to) {
    Range {
    }
}

class Outer {
    class Inner {
        // expect-next-line: empty-constructor
        Inner() {}
    }

    Object anonymous = new Object() {
        { }
    };
}

class Throwing {
    // expect-next-line: empty-constructor
    Throwing() throws Exception {}
}
