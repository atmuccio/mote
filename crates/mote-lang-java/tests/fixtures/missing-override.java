class Point implements Cloneable {
    int x;

    public boolean equals(Object o) { return o == this; }       // expect: missing-override
    public int hashCode() { return x; }                          // expect: missing-override
    public String toString() { return "P"; }                     // expect: missing-override
    protected Point clone() { return this; }                     // expect: missing-override
    protected void finalize() {}                                 // expect: missing-override

    public boolean equals(Point p) { return p.x == x; }         // overload, not an override
    public boolean equals(java.lang.Object o, int depth) { return false; }
    public String toString(int indent) { return ""; }
    public static String toString(Point p) { return ""; }       // static: hides nothing from Object
    public boolean equals(Object[] arr) { return false; }
}

class Annotated {
    @Override
    public boolean equals(java.lang.Object o) { return false; }
    @java.lang.Override
    public int hashCode() { return 1; }
    @Override
    public String toString() { return ""; }
}

interface Shape {
    boolean equals(Object o);                                    // redeclaration without body
    int hashCode();
    String toString();
}

abstract class Base {
    public abstract String toString();
}

enum Color {
    RED {
        public String toString() { return "red"; }               // expect: missing-override
    };
}

record Rec(int a) {
    public String toString() { return "Rec"; }                  // expect: missing-override
}

class Anon {
    Object o = new Object() {
        public int hashCode() { return 7; }                      // expect: missing-override
    };
}
