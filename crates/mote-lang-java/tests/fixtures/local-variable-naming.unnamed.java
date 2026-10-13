import java.util.List;
import java.util.Map;

// Java 22 unnamed variables (`_`) have no name to check.
class Unnamed {
    void run(List<String> items, Map<String, Integer> totals) {
        for (var _ : items) {
            System.out.println("item");
        }
        try (var _ = open()) {
            totals.forEach((_, v) -> System.out.println(v));
        } catch (Exception _) {
            System.out.println("failed");
        }
        int Bad_Name = items.size();                                 // expect: local-variable-naming
        System.out.println(Bad_Name);
    }

    static AutoCloseable open() {
        return () -> {};
    }
}
