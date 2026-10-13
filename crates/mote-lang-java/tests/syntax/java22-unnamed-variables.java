// Java 22: unnamed variables and patterns (`_`).
import java.util.List;
import java.util.Map;
import java.util.function.BiFunction;

class UnnamedVariables {
    sealed interface Shape permits Circle, Square, Point {}

    record Circle(double radius) implements Shape {}

    record Square(double side) implements Shape {}

    record Point(int x, int y) implements Shape {}

    int tally(List<String> items, Map<String, Integer> totals) {
        int n = 0;
        for (var _ : items) {
            n++;
        }
        for (int i = 0, _ = start(); i < 3; i++) {
            n += i;
        }
        int _ = start();
        try (var _ = open()) {
            n++;
        } catch (Exception _) {
            n--;
        }
        totals.forEach((_, v) -> System.out.println(v));
        items.forEach(_ -> n2());
        BiFunction<Integer, Integer, Integer> first = (a, _) -> a;
        return n + first.apply(1, 2);
    }

    static String describe(Shape shape) {
        return switch (shape) {
            case Circle _, Square _ -> "curved or square";
            case Point(int x, _) when x > 0 -> "right half";
            case Point(_, var y) -> "y=" + y;
        };
    }

    static boolean above(Object o) {
        return o instanceof Point(_, int y) && y > 0;
    }

    static int start() {
        return 0;
    }

    static void n2() {}

    static AutoCloseable open() {
        return () -> {};
    }
}
