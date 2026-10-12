import java.util.Map;

class SplitDeclaration {
    String field;

    void positives(Map<String, String> map, boolean flag) {
        String name;                                          // expect: split-declaration
        name = map.get("name");
        final int size;                                       // expect: split-declaration
        // a comment in between does not matter
        size = map.size();
        int[] arr;                                            // expect: split-declaration
        arr = new int[size];
        use(name, arr);
        Runnable r = () -> {
            long t;                                           // expect: split-declaration
            t = System.nanoTime();
        };
    }

    SplitDeclaration(int v) {
        int x;                                                // expect: split-declaration
        x = v * 2;
        use(String.valueOf(x), null);
    }

    void compliant(Map<String, String> map, boolean flag) {
        String value;
        if (flag) {
            value = "a";
        } else {
            value = "b";
        }
        String other;
        try {
            other = map.get("x");
        } catch (RuntimeException e) {
            other = null;
        }
        int a, b;
        a = 1;
        int count;
        count += 1;
        int sum;
        field = "x";
        sum = 2;
        String direct = map.get("y");
        int total;                                            // expect: split-declaration
        total = total(direct);
        Object o;
        this.field = null;
        o = field;
        use(value + other + o, null);
    }

    int total(String s) {
        return 0;
    }

    void use(String s, int[] a) {
    }
}
