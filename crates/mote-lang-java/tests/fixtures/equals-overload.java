class Point {
    int x, y;

    public boolean equals(Point other) {                            // expect: equals-overload
        return other != null && x == other.x && y == other.y;
    }
}

class Money {
    long cents;

    boolean equals(Money m) { return cents == m.cents; }            // expect: equals-overload

    class Inner {
        public boolean equals(Inner o) { return true; }             // expect: equals-overload
    }
}

record Range(int lo, int hi) {
    public boolean equals(Range r) { return lo == r.lo; }           // expect: equals-overload
}

enum Color {
    RED;

    public boolean equals(String name) { return name().equals(name); }  // expect: equals-overload
}

// Compliant ---------------------------------------------------------------
class Proper {
    int v;

    @Override
    public boolean equals(Object o) {
        return o instanceof Proper p && p.v == v;
    }

    @Override
    public int hashCode() { return v; }
}

class TypedHelper {
    int v;

    @Override
    public boolean equals(Object o) {
        return o instanceof TypedHelper t && equals(t);
    }

    public boolean equals(TypedHelper other) { return other.v == v; }   // helper next to the override

    @Override
    public int hashCode() { return v; }
}

interface Comparer<T> {
    boolean equals(T a, T b);
}

class Impl implements Same {
    @Override
    public boolean equals(Impl other) { return true; }              // implements an interface method
}

class Utils {
    static boolean equals(String a) { return a.isEmpty(); }        // static
    int equals(Utils u) { return 0; }                                // not boolean
    boolean equals(java.lang.Object o) { return false; }            // fully qualified Object
}
