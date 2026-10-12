import java.util.Comparator;
import static java.lang.Integer.MIN_VALUE;

class Version implements Comparable<Version> {
    int number;

    @Override
    public int compareTo(Version other) {
        if (other == null) {
            return Integer.MIN_VALUE;                               // expect: compare-to-min-value
        }
        return number == other.number ? 0 : (number < other.number ? java.lang.Integer.MIN_VALUE : 1); // expect: compare-to-min-value
    }
}

class ByName implements Comparator<String> {
    @Override
    public int compare(String a, String b) {
        if (a == null) return MIN_VALUE;                            // expect: compare-to-min-value
        return a.compareTo(b);
    }
}

// Compliant ----------------------------------------------------------------
class Fine implements Comparable<Fine> {
    int n;

    public int compareTo(Fine o) {
        if (o == null) return -1;
        return Integer.compare(n, o.n);
    }

    int lowest() { return Integer.MIN_VALUE; }                      // not a comparison method

    long compare(long a) { return Long.MIN_VALUE; }                 // wrong shape

    Comparator<Fine> lambda() {
        return (x, y) -> {
            return Integer.MAX_VALUE;
        };
    }

    public int compare(Fine a, Fine b) {
        Runnable r = () -> { int v = Integer.MIN_VALUE; };
        return MyInts.MIN_VALUE;                                    // someone else's constant
    }
}
