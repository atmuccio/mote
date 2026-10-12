import lombok.NoArgsConstructor;

public class Strings {                                              // expect: private-constructor-class-not-final
    private Strings() { }

    public static boolean isBlank(String s) {
        return s == null || s.isBlank();
    }
}

class Singleton {                                                   // expect: private-constructor-class-not-final
    private static final Singleton INSTANCE = new Singleton(1);

    private Singleton() { }

    private Singleton(int x) { }

    static class Holder { }                                         // no constructor at all: not reported
}

final class AlreadyFinal {
    private AlreadyFinal() { }
}

abstract class AbstractBase {
    private AbstractBase() { }

    static class Impl extends AbstractBase { }
}

class Extended {
    private Extended() { }

    static final class Sub extends Extended {
        private Sub() { }
    }
}

class AnonymouslyExtended {
    private AnonymouslyExtended() { }

    static AnonymouslyExtended create() {
        return new AnonymouslyExtended() { };
    }
}

class Mixed {
    private Mixed() { }

    Mixed(int x) { }
}

class NoConstructors {
}

sealed class Shape permits Circle {
    private Shape() { }
}

@NoArgsConstructor
class Generated {
    private Generated(int x) { }
}

enum Color {
    RED;

    private Color() { }
}

class Outer {
    private static class Inner {                                    // expect: private-constructor-class-not-final
        private Inner() { }
    }
}
