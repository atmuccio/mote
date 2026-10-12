import java.util.List;
import lombok.Getter;

class Exporter {
    private int written;                                            // expect: field-could-be-local
    private String header;                                          // expect: field-could-be-local
    private int shared;                                             // used by two methods
    private int accumulated;                                        // read before written
    private int conditional;                                        // first write is nested
    private int selfRead;                                           // first write reads itself
    private int captured;                                           // used in a lambda
    private int writeOnly;                                          // never read
    private int counter = 5;                                        // has an initializer
    private static int statics;
    private volatile int flag;
    private int viaThis;                                            // expect: field-could-be-local
    private int recursive;
    private int ctorOnly;
    @Deprecated
    private int annotated;

    Exporter() {
        ctorOnly = 1;
        System.out.println(ctorOnly);
    }

    void export(List<String> rows) {
        written = 0;
        for (String r : rows) {
            written++;
        }
        System.out.println(written);
    }

    String title(String name) {
        header = "# " + name;
        return header;
    }

    void one() {
        shared = 1;
        System.out.println(shared);
    }

    void two() {
        System.out.println(shared);
    }

    void accumulate() {
        System.out.println(accumulated);
        accumulated = 5;
    }

    void maybe(boolean c) {
        if (c) {
            conditional = 1;
        }
        System.out.println(conditional);
    }

    void grow() {
        selfRead = selfRead + 1;
        System.out.println(selfRead);
    }

    void lambda(List<String> rows) {
        captured = 0;
        rows.forEach(r -> System.out.println(captured));
    }

    void store() {
        writeOnly = 3;
    }

    void qualified() {
        this.viaThis = 2;
        System.out.println(this.viaThis);
    }

    void recurse(int depth) {
        recursive = depth;
        if (depth > 0) {
            recurse(depth - 1);
        }
        System.out.println(recursive);
    }

    void touch() {
        counter = 1;
        System.out.println(counter);
        statics = 1;
        System.out.println(statics);
        flag = 1;
        System.out.println(flag);
        annotated = 1;
        System.out.println(annotated);
    }
}

class Shadowing {
    private int value;                                              // the method uses a parameter of the same name

    void set(int value) {
        this.value = value;
    }

    int get() {
        return value;
    }
}

class OtherInstance {
    private int size;

    boolean same(OtherInstance o) {
        size = 1;
        return o.size == size;
    }
}

class ServiceTest {
    private Service service;                                        // set up in a fixture callback

    @BeforeEach
    void setUp() {
        service = new Service();
        service.start();
    }
}

@Getter
class Generated {
    private int value;

    void m() {
        value = 1;
        System.out.println(value);
    }
}
