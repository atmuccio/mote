public class Report {                                               // expect-next-line: redundant-default-constructor
    public Report() {
    }

    public String render() {
        return "";
    }
}

class PackagePrivate {                                              // expect-next-line: redundant-default-constructor
    PackagePrivate() {
        super();
    }
}

class Outer {
    protected static class Nested {                                 // expect-next-line: redundant-default-constructor
        protected Nested() { }
    }

    private static class Hidden {                                   // expect-next-line: redundant-default-constructor
        private Hidden() { }
    }
}

public class DifferentAccess {
    DifferentAccess() { }                                           // default one would be public
}

public class Utility {
    private Utility() { }                                           // prevents instantiation
}

class Documented {
    /** Needed by the persistence framework. */
    Documented() { }
}

class Commented {
    Commented() {
        // intentionally empty
    }
}

class Annotated {
    @Inject
    Annotated() { }
}

class Throwing {
    Throwing() throws Exception { }
}

class WithBody {
    WithBody() {
        init();
    }

    void init() { }
}

class SuperWithArgs extends Base {
    SuperWithArgs() {
        super(1);
    }
}

class TwoConstructors {
    TwoConstructors() { }

    TwoConstructors(int x) { }
}

class WithParams {
    WithParams(int x) { }
}

enum Color {
    RED;

    Color() { }
}

class TrailingComment {
    TrailingComment() { } // required by the persistence framework
}
