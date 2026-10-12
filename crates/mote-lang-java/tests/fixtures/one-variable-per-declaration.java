import java.util.List;

class OneVariablePerDeclaration {
    private int width, height;                                // expect: one-variable-per-declaration
    private int a, b[];                                       // expect: one-variable-per-declaration
    static final String X = "x", Y = "y", Z = "z";            // expect: one-variable-per-declaration
    private int single;
    private final List<String> names = List.of("a", "b");

    interface Constants {
        int LOW = 1, HIGH = 2;                                // expect: one-variable-per-declaration
        int ONE = 1;
    }

    void method(List<String> items) {
        String first = null, last = null;                     // expect: one-variable-per-declaration
        int i, j;                                             // expect: one-variable-per-declaration
        for (int k = 0, n = items.size(); k < n; k++) {       // the for header is exempt
            int x = k;
        }
        for (String s : items) {
            int len = s.length();
        }
        try (var in = new java.io.StringReader("a"); var out = new java.io.StringWriter()) {
            int ok = 0;
        } catch (java.io.IOException e) {
            int ignored = 1;
        }
        Runnable r = () -> {
            int p = 1, q = 2;                                 // expect: one-variable-per-declaration
        };
    }
}
