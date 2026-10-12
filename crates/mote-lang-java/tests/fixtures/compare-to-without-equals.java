import com.example.BaseEntity;
import java.util.Objects;

class Version implements Comparable<Version> {                    // expect: compare-to-without-equals
    int major;

    @Override
    public int compareTo(Version o) {
        return Integer.compare(major, o.major);
    }
}

class Priority extends Number implements Comparable<Priority> {   // expect: compare-to-without-equals
    int level;

    public int compareTo(Priority o) {
        return level - o.level;
    }

    public int intValue() { return level; }
    public long longValue() { return level; }
    public float floatValue() { return level; }
    public double doubleValue() { return level; }
}

class Base {
    int id;
}

class Derived extends Base implements Comparable<Derived> {       // expect: compare-to-without-equals
    public int compareTo(Derived o) {
        return Integer.compare(id, o.id);
    }
}

// ---- compliant --------------------------------------------------------------------------

class WithEquals implements Comparable<WithEquals> {
    int v;

    public int compareTo(WithEquals o) {
        return Integer.compare(v, o.v);
    }

    @Override
    public boolean equals(Object o) {
        return o instanceof WithEquals w && w.v == v;
    }

    @Override
    public int hashCode() {
        return Objects.hash(v);
    }
}

class BaseWithEquals {
    @Override
    public boolean equals(Object o) {
        return false;
    }
}

class InheritsEquals extends BaseWithEquals implements Comparable<InheritsEquals> {
    public int compareTo(InheritsEquals o) {
        return 0;
    }
}

class ExternalParent extends BaseEntity implements Comparable<ExternalParent> {
    public int compareTo(ExternalParent o) {
        return 0;
    }
}

abstract class AbstractItem implements Comparable<AbstractItem> {
    public int compareTo(AbstractItem o) {
        return 0;
    }
}

record Point(int x) implements Comparable<Point> {
    public int compareTo(Point o) {
        return Integer.compare(x, o.x);
    }
}

enum Level implements Comparable<Level> {
    LOW;
}

@lombok.EqualsAndHashCode
class Generated implements Comparable<Generated> {
    int v;

    public int compareTo(Generated o) {
        return 0;
    }
}

class NoCompareTo implements Comparable<NoCompareTo>, Runnable {
    public void run() {}

    public int compareTo(NoCompareTo a, NoCompareTo b) {
        return 0;
    }
}
