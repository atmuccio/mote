import com.acme.Record;
import java.lang.annotation.Annotation;
import java.lang.reflect.Method;
import java.util.ArrayList;
import java.util.Collection;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

class Shapes {
    private final List<Integer> numbers = new ArrayList<>();

    List<? extends Number> numbers() {                                       // expect: wildcard-return-type
        return numbers;
    }

    public Map<String, ? super Integer> counts() {                           // expect: wildcard-return-type
        Map<String, Integer> counts = new HashMap<>();
        if (numbers.isEmpty()) {
            return counts;
        }
        return counts;
    }

    private static List<? extends Record> records() {                        // expect: wildcard-return-type
        List<Record> out = new ArrayList<>();
        return out;
    }

    private static List<? extends CharSequence> created() {                  // expect: wildcard-return-type
        return new ArrayList<String>();
    }

    <T extends String> T first(List<T> items) {                              // expect: wildcard-return-type
        return items.get(0);
    }

    static class Box<V extends Integer> {                                    // expect: wildcard-return-type
    }

    final static class Leaf {
    }

    <L extends Leaf> void leaf(L l) {                                        // expect: wildcard-return-type
    }

    enum Color { RED }

    <C extends Color> void color(C c) {                                      // expect: wildcard-return-type
    }

    // Compliant
    private final List<? extends Number> mixed = new ArrayList<Integer>();
    private final List<Integer> ints = new ArrayList<>();
    private final List<Double> doubles = new ArrayList<>();

    List<?> any() {
        return null;
    }

    Class<? extends Annotation> annotationType() {                          // idiomatic for Class
        return null;
    }

    List<Class<? extends Number>> classes() {
        return null;
    }

    List<? extends Object> objects() {
        return ints;
    }

    void accept(List<? extends Number> in) {                                 // parameters are fine
    }

    List<? extends Number> fromWildcardField() {                             // the value's own type has it
        return mixed;
    }

    List<? extends Number> either(boolean b) {                               // two parameterizations
        if (b) {
            return ints;
        }
        return doubles;
    }

    List<? extends Number> diamond() {                                       // type from the context
        return new ArrayList<>();
    }

    List<? extends Number> computed() {                                      // type not visible
        return java.util.Collections.unmodifiableList(ints);
    }

    List<? extends Number> viaVar() {
        var local = new ArrayList<Integer>();
        return local;
    }

    <T extends Comparable<T>> T max(List<T> items) {
        return null;
    }

    <T extends Number & Comparable<T>> T both(List<T> items) {              // several bounds
        return null;
    }

    static class Node<T extends Node<T>> {
    }
}

interface Authorities {
    Collection<? extends CharSequence> authorities();                        // implementations may narrow it
}

class User implements Authorities {
    private final List<String> names = new ArrayList<>();

    public Collection<? extends CharSequence> authorities() {                 // implements a chosen signature
        return names;
    }

    @Override
    public String toString() {
        return "";
    }
}

class Child extends Shapes {
    private final List<Integer> mine = new ArrayList<>();

    List<? extends Number> numbers() {                                        // may override
        return mine;
    }

    private List<? extends Number> helper() {                                 // expect: wildcard-return-type
        return mine;
    }
}

abstract class Records {
    private final List<Integer> values = new ArrayList<>();

    abstract List<? extends Number> values();                                  // overridden with a narrower type

    static <T> java.lang.reflect.Constructor<? extends T> constructor(Class<T> c) {  // a reflective handle
        return null;
    }

    final List<? extends Number> frozen() {                                    // expect: wildcard-return-type
        return values;
    }
}

class Anonymous {
    Object make() {
        return new Object() {
            private final List<Integer> xs = new ArrayList<>();

            List<? extends Number> numbers() {
                return xs;
            }
        };
    }
}
