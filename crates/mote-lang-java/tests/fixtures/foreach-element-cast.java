import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Set;

class ForeachElementCast {
    void names(List<String> names, String[] array, Set<Map.Entry<String, Integer>> entries) {
        for (Object o : names) {                                        // expect: foreach-element-cast
            String name = (String) o;
            print(name);
        }
        for (CharSequence cs : array) {                                 // expect: foreach-element-cast
            String s = (String) cs;
            print(s);
        }
        for (Object e : entries) {                                      // expect: foreach-element-cast
            Map.Entry<String, Integer> entry = (Map.Entry<String, Integer>) e;
            print(entry.getKey());
        }
        ArrayList<Integer> numbers = new ArrayList<>();
        for (Number n : numbers) {                                      // expect: foreach-element-cast
            final Integer i = ((Integer) (n));
            print(i.toString());
        }
    }

    // Compliant
    @SuppressWarnings("rawtypes")
    void fine(List raw, List<Number> numbers, List<String> names, List<?> unknown) {
        for (Object o : raw) {                                          // raw: the cast is needed
            String s = (String) o;
            print(s);
        }
        for (Number n : numbers) {                                      // narrows beyond the element type
            Integer i = (Integer) n;
            print(i.toString());
        }
        for (String s : names) {
            print(s);
        }
        for (Object o : names) {
            print(o.toString());                                        // not cast first
            String s = (String) o;
        }
        for (Object o : unknown) {
            String s = (String) o;
        }
        for (Object o : names) {
            String a = (String) o, b = "x";
        }
    }

    static void print(String s) {
    }
}
