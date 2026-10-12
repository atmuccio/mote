import java.util.List;
import java.util.Map;

class Sizes {
    void wrong(List<String> items, Map<String, Integer> map, String name, int[] values, StringBuilder sb) {
        if (items.size() < 0) { }                   // expect: impossible-size-check
        if (name.length() >= 0) { }                 // expect: impossible-size-check
        if (values.length == -1) { }                // expect: impossible-size-check
        if (map.size() > -1) { }                    // expect: impossible-size-check
        if (0 > sb.length()) { }                    // expect: impossible-size-check
        if (items.size() <= -1) { }                 // expect: impossible-size-check
        boolean b = items == null || items.size() < 0;   // expect: impossible-size-check
    }

    void fine(List<String> items, String name, int[] values, Custom custom, int n) {
        if (items.size() > 0) { }
        if (items.size() == 0) { }
        if (name.length() >= 1) { }
        if (values.length <= 0) { }
        if (name.indexOf('x') == -1) { }
        if (custom.size() < 0) { }
        if (n < 0) { }
        if (items.size() < n) { }
        if (name.length() != 0) { }
    }

    interface Custom { int size(); }
}
