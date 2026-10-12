import java.util.List;
import java.util.function.Supplier;

class Util {
    static <T> void clear(List<?> items) {                        // expect: unused-type-parameter
        items.clear();
    }

    private <K, V> int size(List<K> keys) {                        // expect: unused-type-parameter
        return keys.size();
    }

    <T> Util() { }                                                 // expect: unused-type-parameter

    // Used in the signature, the body, a bound, a cast or throws: fine.
    static <T> T first(List<T> items) { return items.get(0); }
    static <T> void body(Object o) { List<T> copy = null; }
    static <T, L extends List<T>> L same(L list) { return list; }
    @SuppressWarnings("unchecked") static <T> Object cast(Object o) { return (T) o; }
    static <E extends Exception> void run(Runnable r) throws E { r.run(); }
    static <T> Supplier<T> lazy(Supplier<T> s) { return s; }

    // Instance method of a class without supertypes: reported.
    <T> void log(String message) { }                               // expect: unused-type-parameter

    // Abstract / no body: not reported.
    abstract static class Visitor {
        abstract <R> void visit(Object node);
    }
}

class Impl implements Comparable<Impl> {
    // Could implement an interface method: not reported.
    <T> void maybeOverride() { }

    @Override
    public int compareTo(Impl o) { return 0; }

    // Static: reported even with supertypes.
    static <T> void helper() { }                                   // expect: unused-type-parameter
}

class Child extends Util {
    @Override
    <T> void log(String message) { }
}

interface Api {
    default <T> void hook() { }                                    // expect: unused-type-parameter
    <T> void declared();
}

interface SubApi extends Api {
    default <T> void other() { }
}

class Anon {
    Object o = new Object() {
        <T> void inAnonymous() { }
    };
}
