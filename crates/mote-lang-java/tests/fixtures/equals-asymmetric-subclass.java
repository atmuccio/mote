import java.util.Objects;

class Point {
    int x, y;

    @Override
    public boolean equals(Object o) {
        return o instanceof Point p && x == p.x && y == p.y;
    }

    @Override
    public int hashCode() { return Objects.hash(x, y); }
}

class ColorPoint extends Point {
    int color;

    @Override
    public boolean equals(Object o) {
        if (!(o instanceof ColorPoint)) {                   // expect: equals-asymmetric-subclass
            return false;
        }
        ColorPoint c = (ColorPoint) o;
        return super.equals(c) && color == c.color;
    }
}

class Shape<T> {
    T id;

    public boolean equals(Object other) {
        if (!(other instanceof Shape)) return false;
        return Objects.equals(id, ((Shape<?>) other).id);
    }
}

class Circle<T> extends Shape<T> {
    double r;

    public boolean equals(Object other) {
        return other instanceof Circle<?> c && super.equals(c) && r == c.r;   // expect: equals-asymmetric-subclass
    }
}

// Negatives.
class Base {
    int a;

    public boolean equals(Object o) {
        if (o == null || getClass() != o.getClass()) return false;
        return a == ((Base) o).a;
    }
}

class Derived extends Base {
    int b;

    public boolean equals(Object o) {
        return o instanceof Derived d && super.equals(d) && b == d.b;
    }
}

class Money {
    long cents;

    public boolean equals(Object o) {
        return o instanceof Money m && m.canEqual(this) && cents == m.cents;
    }

    boolean canEqual(Object o) { return o instanceof Money; }
}

class Euro extends Money {
    public boolean equals(Object o) {
        return o instanceof Euro e && e.canEqual(this) && cents == e.cents;
    }

    boolean canEqual(Object o) { return o instanceof Euro; }
}

class Plain extends Point {
    int z;                                                  // no equals of its own
}

class Unrelated extends java.util.ArrayList<String> {
    public boolean equals(Object o) {
        return o instanceof Unrelated && super.equals(o);  // superclass not in this file
    }
}
