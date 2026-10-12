import java.util.Objects;

class OnlyEquals {
    int x;

    @Override
    public boolean equals(Object o) {                               // expect: equals-without-hashcode
        return o instanceof OnlyEquals && ((OnlyEquals) o).x == x;
    }
}

class OnlyHashCode {
    @Override
    public int hashCode() { return 42; }                            // expect: equals-without-hashcode
}

class Both {
    int x;
    @Override public boolean equals(Object o) { return o == this; }
    @Override public int hashCode() { return Objects.hash(x); }
}

class OverloadedEquals {
    public boolean equals(OverloadedEquals other) { return other != null; }
}

class StaticHelpers {
    static boolean equals(Object a) { return false; }
    static int hashCode(int v) { return v; }
}

record Point(int x, int y) {
    @Override public boolean equals(Object o) { return o instanceof Point; }
}

// Lombok generates neither method when the class writes one of them.
@lombok.EqualsAndHashCode
class LombokManaged {
    @Override public boolean equals(Object o) { return true; }     // expect: equals-without-hashcode
}

@lombok.Data
class LombokDelegating extends Base {
    @Override public boolean equals(Object o) { return super.equals(o); } // keeps Base's equality
}

class Delegating extends Base {
    @Override public boolean equals(Object o) { return super.equals(o); }
    @Override public int hashCode() { return 31 * super.hashCode(); }
}

class DelegatingHash extends Base {
    @Override public int hashCode() { return super.hashCode(); }
}

@lombok.EqualsAndHashCode
class LombokGenerated {
    private int id;
}

@lombok.Data
class LombokBoth {
    private int id;
    @Override public boolean equals(Object o) { return true; }
    @Override public int hashCode() { return id; }
}

class Outer {
    static class Inner {
        public boolean equals(java.lang.Object o) { return false; }   // expect: equals-without-hashcode
    }

    public int hashCode() { return 1; }                               // expect: equals-without-hashcode

    Object anonymous = new Object() {
        @Override public boolean equals(Object o) { return false; }
    };
}

enum Color { RED }

interface Shape {
    boolean equals(Object o);
}

abstract class Base {
    public abstract boolean equals(Object o);
    public abstract int hashCode();
}
