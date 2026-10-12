import java.util.List;

class RedundantParentheses {
    int count;
    int[] values = new int[4];

    int returns(int a, int b, boolean ok) {
        if (ok) {
            return (count);                                   // expect: redundant-parentheses
        }
        if (a > b) {
            return (a + b);                                   // expect: redundant-parentheses
        }
        if (a < 0) {
            return (ok ? a : b);                              // a conditional: parenthesized on purpose
        }
        return ((a * b));                                     // expect: redundant-parentheses
    }

    void throwsIt(RuntimeException e) {
        throw (e);                                            // expect: redundant-parentheses
    }

    int yields(int k) {
        return switch (k) {
            case 1 -> {
                yield (k + 1);                                // expect: redundant-parentheses
            }
            default -> k;
        };
    }

    void operands(String name, List<String> list, int i, boolean ready) {
        String label = (name).trim();                         // expect: redundant-parentheses
        count += (i);                                         // expect: redundant-parentheses
        int v = values[(i)];                                  // expect: redundant-parentheses
        String s = "a" + (name.length());                     // expect: redundant-parentheses
        Object o = (new StringBuilder()).append(1);           // expect: redundant-parentheses
        if ((ready)) {                                        // expect: redundant-parentheses
            count++;
        }
        while ((i > 0)) {                                     // expect: redundant-parentheses
            i--;
        }
        Runnable r = () -> foo((1));                          // expect: redundant-parentheses
    }

    void compliant(int a, int b, int c, boolean p, boolean q, boolean r, Object obj, int[] arr) {
        int x = a + (b * c);
        boolean y = (p && q) || r;
        boolean empty = (a == 0);
        int z = (a + b) * c;
        String t = (String) obj;
        int u = -(a + b);
        int w = (int) (a / 2.0);
        Object first = ((Object[]) obj)[0];
        int len = (new int[3])[0];
        if (a > 0) {
            count = a;
        }
        synchronized (this) {
            count++;
        }
        switch (a) {
            case 1 -> count++;
            default -> count--;
        }
        int commented = (/* why */ a);
        foo(a > 0 ? a : -a);
        Runnable run = () -> foo(a);
    }

    void foo(int v) {
    }

    enum Part { HEADER, FOOTER }

    void lambdaInArrowCase(Part part, char c) {
        java.util.function.Function<String, Runnable> fun;
        // The parser reads `HEADER -> fun = (s)` as a lambda in the case label: `(s)` is the
        // real lambda's parameter list, not parentheses around a name.
        switch (part) {
            case HEADER -> fun = (s) -> s::trim;
            default -> fun = (s) -> s::strip;
        }
        switch (c) {
            case ('I'):                                       // expect: redundant-parentheses
                count++;
                break;
            default:
                break;
        }
    }

    boolean ret(int a, int b) {
        return (a = b) > 0;
    }
}
