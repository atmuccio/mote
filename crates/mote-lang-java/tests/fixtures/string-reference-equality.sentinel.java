import java.util.Map;

class Lookup {
    private static final String NOT_FOUND = new String("__missing__");
    private final String unset = new java.lang.String();
    private static final String PLAIN = "__missing__";

    String find(Map<String, String> m, String key) {
        String v = m.getOrDefault(key, NOT_FOUND);
        if (v == NOT_FOUND) {
            return null;
        }
        if (this.unset != v) {
            return v;
        }
        String marker = new String("x");
        if (marker == v) {
            return "";
        }
        if (v == PLAIN) {                           // expect: string-reference-equality
            return "";
        }
        return v;
    }
}
