public abstract class JdbcHelpers {                                 // expect: abstract-class-without-methods
    public static final String DRIVER = "org.h2.Driver";

    private JdbcHelpers() {
    }
}

abstract class Shape {
    private Shape() {
    }

    static final class Circle extends Shape {
    }
}

abstract class Token {
    private Token() {
    }

    static final Token EMPTY = new Token() { };
}

abstract class Visible {
    Visible() {
    }
}
