import java.util.Objects;

class Order {
    private long id;

    public int hashcode() {                                 // expect: object-method-name-typo
        return Objects.hash(id);
    }

    public String tostring() {                              // expect: object-method-name-typo
        return "Order " + id;
    }

    public boolean Equals(Object other) {                   // expect: object-method-name-typo
        return other instanceof Order o && o.id == id;
    }
}

class Line {
    long id;

    public boolean equal(Object other) {                    // expect: object-method-name-typo
        return other instanceof Line l && l.id == id;
    }

    public String ToString() {                              // expect: object-method-name-typo
        return "Line";
    }
}

class Fine {
    long id;

    @Override
    public int hashCode() { return Long.hashCode(id); }

    public int hashcode() { return hashCode(); }            // the real one exists too

    public boolean equal(Fine other) { return other.id == id; }

    public boolean equal(Object a, Object b) { return Objects.equals(a, b); }

    public static String tostring() { return "static helper"; }

    public String tostring(int indent) { return ""; }

    public long hashcode(int seed) { return seed; }

    public void equals() { }
}
