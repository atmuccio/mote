import java.io.*;
import java.util.*;
import java.util.function.*;

class Locals {
    int field;

    int basic(List<String> items) {
        int total = items.size();
        String label = "x";                              // expect: unused-local-variable
        int a = 1, b = 2;                                // expect: unused-local-variable
        return total + a;
    }

    void writeOnly() {
        int count = 0;                                   // expect: unused-local-variable
        count = 5;
        count += 2;
        count++;
        --count;
        int steps = 0;                                   // expect: unused-local-variable
        for (int i = 0; i < 3; i++, steps++) {
            field++;
        }
    }

    int readViaCompound() {
        int x = 0;
        int y = (x += 3);          // compound used as a value: reads x
        int z = 0;
        return y + z++;            // z++ inside an expression reads z
    }

    void loops(int[] data) {
        for (int i = 0, n = data.length; i < n; i++) {
            System.out.println(data[i]);
        }
        for (int k = 0; ; k++) {                         // expect: unused-local-variable
            break;
        }
        for (int d : data) {       // enhanced-for variables are not reported
        }
    }

    void lambdasAndClasses() {
        int captured = 1;
        Runnable r = () -> System.out.println(captured);
        r.run();
        int inAnon = 2;
        Object o = new Object() {
            @Override
            public String toString() {
                return "" + inAnon;
            }
        };
        o.hashCode();
        Supplier<Integer> s = () -> {
            int inner = 3;                               // expect: unused-local-variable
            return 4;
        };
        s.get();
        Function<Integer, Integer> f = x -> x + 1;
        f.apply(1);
    }

    void scopes() {
        {
            int shadow = 1;                              // expect: unused-local-variable
        }
        {
            int shadow = 2;
            System.out.println(shadow);
        }
        field = 3;
        int field = 4;                                   // expect: unused-local-variable
        this.field = 5;
    }

    void switches(int v) {
        switch (v) {
            case 1:
                int shared = 1;
                break;
            case 2:
                shared = 2;
                System.out.println(shared);
                break;
            default:
                int lonely = 3;                          // expect: unused-local-variable
        }
        switch (v) {
            case 1 -> {
                int arrow = 1;                           // expect: unused-local-variable
            }
            default -> {
                int arrow = 2;
                System.out.println(arrow);
            }
        }
    }

    void skipped(Object o) throws Exception {
        @SuppressWarnings("unused") int annotated = 1;
        int ignored = 2;
        int unusedValue = 3;
        int dummy = 4;
        try (InputStream in = new FileInputStream("f")) {
        } catch (IOException e) {
        }
        if (o instanceof String str) {
        }
        var _ = compute();
    }

    int methodNameIsNotARead() {
        int compute = 1;                                 // expect: unused-local-variable
        return compute();
    }

    int compute() { return 0; }

    void arrays() {
        int[] arr = new int[3];
        arr[0] = 1;               // writes an element but reads the array reference
        StringBuilder sb = new StringBuilder();
        sb.append("x");
        Object ref = null;
        ref = sb;
        use(ref);
    }

    void use(Object o) {}

    void methodRef(List<String> list) {
        Consumer<String> printer = System.out::println;
        list.forEach(printer);
        List<String> copy = new ArrayList<>();
        list.forEach(copy::add);
    }

    void resourceByReference(InputStream in) throws IOException {
        InputStream alias = in;
        try (alias) {
            in.read();
        }
    }
}

record Point(int x, int y) {
    Point {
        int check = x + y;                               // expect: unused-local-variable
    }
}

enum Mode {
    ON;
    void m() {
        String tmp = name();                             // expect: unused-local-variable
    }
}
