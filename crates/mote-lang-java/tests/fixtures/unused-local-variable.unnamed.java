import java.util.List;
import java.util.Map;

// Java 22 unnamed variables (`_`) are unused by design and never reported.
class Tally {
    int count(List<String> items, Map<String, Integer> totals) {
        int n = 0;
        for (var _ : items) {
            n++;
        }
        int _ = reset();
        try (var _ = open()) {
            n++;
        } catch (Exception _) {
            n--;
        }
        totals.forEach((_, v) -> System.out.println(v));
        int spare = reset();                                         // expect: unused-local-variable
        return n;
    }

    static boolean right(Object o) {
        return o instanceof Point(int x, _) && x > 0;
    }

    record Point(int x, int y) {}

    static int reset() {
        return 0;
    }

    static AutoCloseable open() {
        return () -> {};
    }
}
