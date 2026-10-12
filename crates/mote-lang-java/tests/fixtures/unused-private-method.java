import java.io.*;
import java.util.*;
import java.util.function.*;

class Service implements Serializable {
    public String render() {
        return header() + helper(1);
    }

    private String header() { return "#"; }
    private String footer() { return "--"; }                       // expect: unused-private-method

    private String helper(int x) { return "" + x; }
    private String helper() { return ""; }                         // overloads: matched by name

    private int recurse(int n) {                                   // expect: unused-private-method
        return n <= 0 ? 0 : recurse(n - 1);
    }

    private void byReference() {}
    private static int parse(String s) { return 0; }
    private void byThis() {}
    Runnable r = this::byThis;
    Function<String, Integer> p = Service::parse;

    void run() {
        List<Runnable> tasks = new ArrayList<>();
        tasks.add(this::byReference);
    }

    private void calledFromLambda() {}
    Runnable lambda = () -> calledFromLambda();

    private void calledFromInner() {}
    class Inner {
        void go() { calledFromInner(); }
    }

    private void calledFromAnonymous() {}
    Runnable anon = new Runnable() {
        public void run() { calledFromAnonymous(); }
    };

    // Serialization hooks are called by the JVM.
    private void writeObject(ObjectOutputStream out) throws IOException {}
    private void readObject(ObjectInputStream in) throws IOException {}
    private void readObjectNoData() {}
    private Object writeReplace() { return this; }
    private Object readResolve() { return this; }

    // Annotated methods are framework callbacks or explicitly suppressed.
    @SuppressWarnings("unused")
    private void suppressed() {}
    @javax.annotation.PostConstruct
    private void init() {}

    // Reflection.
    private void reflective() {}
    Object m() throws Exception {
        return getClass().getDeclaredMethod("reflective");
    }

    private void calledOnOtherReceiver() {}
    void other(Service s) { s.calledOnOtherReceiver(); }

    public void notPrivate() {}
    protected void alsoNotPrivate() {}

    static class Nested {
        private void nestedUnused() {}                             // expect: unused-private-method
        private void nestedUsedByOuter() {}
    }
    void outerCallsNested(Nested n) { n.nestedUsedByOuter(); }

    interface Api {
        private void unusedInInterface() {}                        // expect: unused-private-method
        default void run() {}
    }

    enum Mode {
        ON;
        private int unusedInEnum() { return 1; }                   // expect: unused-private-method
    }

    record Rec(int a) {
        private int unusedInRecord() { return a; }                 // expect: unused-private-method
    }
}

class JUnitStyle {
    @org.junit.jupiter.params.ParameterizedTest
    @org.junit.jupiter.params.provider.MethodSource
    void cases(int x) {}
    private static List<Integer> cases() { return List.of(1); }

    @org.junit.jupiter.params.ParameterizedTest
    @org.junit.jupiter.params.provider.MethodSource("com.acme.Data#rows")
    void external(int x) {}
    private static List<Integer> rows() { return List.of(1); }
}
