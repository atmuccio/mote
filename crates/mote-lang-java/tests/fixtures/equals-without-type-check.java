import java.util.Objects;

class Point {
    int x, y;

    @Override
    public boolean equals(Object o) {
        Point p = (Point) o;                                        // expect: equals-without-type-check
        return x == p.x && y == p.y;
    }

    @Override
    public int hashCode() { return Objects.hash(x, y); }
}

class Named {
    String name;

    @Override
    public boolean equals(Object other) {
        if (other == this) return true;
        if (other == null) return false;
        return name.equals(((Named) other).name);                   // expect: equals-without-type-check
    }

    @Override
    public int hashCode() { return name.hashCode(); }
}

// Compliant ---------------------------------------------------------------------
class WithInstanceof {
    int v;

    public boolean equals(Object o) {
        if (!(o instanceof WithInstanceof)) return false;
        WithInstanceof w = (WithInstanceof) o;
        return w.v == v;
    }

    public int hashCode() { return v; }
}

class WithGetClass {
    int v;

    public boolean equals(Object o) {
        if (o == null || getClass() != o.getClass()) return false;
        return ((WithGetClass) o).v == v;
    }

    public int hashCode() { return v; }
}

class WithSuper extends Base {
    int v;

    public boolean equals(Object o) {
        if (!super.equals(o)) return false;                         // the superclass checks the type
        return ((WithSuper) o).v == v;
    }

    public int hashCode() { return v; }
}

class WithCatch {
    int v;

    public boolean equals(Object o) {
        try {
            return ((WithCatch) o).v == v;
        } catch (ClassCastException e) {
            return false;
        }
    }

    public int hashCode() { return v; }
}

class WithSwitch {
    int v;

    public boolean equals(Object o) {
        return switch (o) {
            case WithSwitch w -> w.v == v;
            default -> false;
        };
    }

    public int hashCode() { return v; }
}

class NotObjectParam {
    int v;

    public boolean equals(NotObjectParam o) {
        return ((NotObjectParam) o).v == v;
    }

    public boolean equals(Object a, Object b) {
        return ((NotObjectParam) a).v == v;
    }
}

class CastsSomethingElse {
    Object field;

    public boolean equals(Object o) {
        return o != null && ((String) field).isEmpty();
    }

    public int hashCode() { return 0; }
}
