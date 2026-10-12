import java.util.Comparator;
import java.util.List;

final class Version {
    int major;

    public int compareTo(Version other) {                   // expect: compare-to-without-comparable
        return Integer.compare(major, other.major);
    }
}

record Range(int lo, int hi) {
    int compareTo(Range o) {                                // expect: compare-to-without-comparable
        return Integer.compare(lo, o.lo);
    }
}

class Box<T> {
    T value;

    public int compareTo(Box<T> other) {                    // expect: compare-to-without-comparable
        return 0;
    }
}

// Negatives.
final class Good implements Comparable<Good> {
    public int compareTo(Good o) { return 0; }
}

class Sub extends Base {
    public int compareTo(Sub o) { return 0; }               // the base class may be Comparable
}

class Base {}

class Other {
    public int compareTo(String s) { return 0; }            // not its own type
    static int compareTo(Other a) { return 0; }
}

class UsedAsComparator {
    public int compareTo(UsedAsComparator o) { return 0; }

    static void sort(List<UsedAsComparator> list) {
        list.sort(UsedAsComparator::compareTo);
    }
}

interface Shape {
    int compareTo(Shape s);
}
