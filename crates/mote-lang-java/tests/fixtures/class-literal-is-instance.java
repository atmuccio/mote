import java.util.List;
import java.util.stream.Collectors;

class ClassLiteralIsInstance {
    double total(List<Object> values) {
        double total = 0;
        for (Object value : values) {
            if (Number.class.isInstance(value)) {                           // expect: class-literal-is-instance
                total += ((Number) value).doubleValue();
            }
        }
        boolean a = String[].class.isInstance(values.get(0));               // expect: class-literal-is-instance
        boolean b = java.util.Map.class.isInstance(values);                 // expect: class-literal-is-instance
        long c = values.stream().filter(v -> CharSequence.class.isInstance(v)).count(); // expect: class-literal-is-instance
        return total;
    }

    // Compliant
    <T> List<T> ofType(List<Object> values, Class<T> type) {
        boolean a = type.isInstance(values.get(0));                        // a Class only known at run time
        boolean b = int.class.isInstance(values.get(0));                   // always false: another problem
        boolean c = values.get(0) instanceof Number;
        return values.stream().filter(type::isInstance).map(type::cast).collect(Collectors.toList());
    }

    List<Number> numbers(List<Object> values) {
        return values.stream().filter(Number.class::isInstance).map(Number.class::cast).collect(Collectors.toList());
    }

    static class Checker {
        boolean isInstance(Object o) { return true; }
    }

    boolean other(Checker k, Object o) {
        return k.isInstance(o);
    }
}
