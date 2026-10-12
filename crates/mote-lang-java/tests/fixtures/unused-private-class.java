import java.util.*;

public class Outer {
    private static class Unused {}                                 // expect: unused-private-class
    private static class SelfReferencing {                         // expect: unused-private-class
        SelfReferencing next;
        static SelfReferencing create() { return new SelfReferencing(); }
    }
    private interface UnusedApi {}                                 // expect: unused-private-class
    private enum UnusedMode { A, B }                               // expect: unused-private-class
    private record UnusedRec(int a) {}                             // expect: unused-private-class

    private static class UsedAsType {}
    private final List<UsedAsType> items = new ArrayList<>();

    private static class UsedByNew {}
    Object make() { return new UsedByNew(); }

    private static class UsedStatically {
        static final int VALUE = 1;
    }
    int value() { return UsedStatically.VALUE; }

    private static class UsedByMethodRef {
        UsedByMethodRef() {}
    }
    java.util.function.Supplier<Object> s = UsedByMethodRef::new;

    private static class UsedInCatch extends RuntimeException {}
    void run() {
        try {
            run();
        } catch (UsedInCatch e) {
        }
    }

    private static class UsedByClassLiteral {}
    Class<?> k = UsedByClassLiteral.class;

    private enum UsedEnum { X }
    UsedEnum e = UsedEnum.X;

    private static class UsedReflectively {}
    Object load() throws Exception { return Class.forName("com.acme.Outer$UsedReflectively"); }

    private static class Base {}
    private static class Derived extends Base {}                   // expect: unused-private-class

    @SuppressWarnings("unused")
    private static class Suppressed {}

    private class InnerUsedAsGeneric {}
    Map<String, InnerUsedAsGeneric> map;

    static class NotPrivate {}
    protected static class AlsoNotPrivate {}

    private static class Container {
        private static class Nested {}
        Nested n;
    }
    Container c;
}
