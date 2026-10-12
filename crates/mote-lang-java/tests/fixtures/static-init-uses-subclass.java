import java.util.List;

public abstract class StaticInitUsesSubclass {
    public static final StaticInitUsesSubclass DEFAULT = new Utf8();              // expect: static-init-uses-subclass
    static final StaticInitUsesSubclass LATIN;
    static final StaticInitUsesSubclass BUILT = Latin.create();                   // expect: static-init-uses-subclass
    static final List<StaticInitUsesSubclass> ALL = List.of(new Deep());          // expect: static-init-uses-subclass

    static {
        LATIN = new Latin();                                                       // expect: static-init-uses-subclass
    }

    // Compliant
    static final StaticInitUsesSubclass ANON = new StaticInitUsesSubclass() { };  // anonymous
    static final StaticInitUsesSubclass HIDDEN = new Hidden();                     // private subclass
    static final Runnable LAZY = () -> new Utf8();                                 // deferred
    static final Other OTHER = new Other();                                        // not a subclass
    final StaticInitUsesSubclass instance = new Utf8();                            // not static

    static StaticInitUsesSubclass make() {
        return new Utf8();                                                          // a method, not initialization
    }

    private static final class Hidden extends StaticInitUsesSubclass { }

    static class Other { }
}

class Utf8 extends StaticInitUsesSubclass { }

class Latin extends StaticInitUsesSubclass {
    static Latin create() { return new Latin(); }
}

class Deep extends Latin { }

final class Holder {
    static final StaticInitUsesSubclass DEFAULT = new Utf8();                     // Holder is not a superclass
}

enum Mode {
    A { },
    B;
    static final Mode FIRST = A;
}
