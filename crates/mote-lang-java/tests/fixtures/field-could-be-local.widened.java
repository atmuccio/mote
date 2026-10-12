import java.util.List;
import lombok.Data;
import lombok.extern.slf4j.Slf4j;

class Panel {
    private String title = null;                                    // expect: field-could-be-local
    private boolean dirty = false;                                  // expect: field-could-be-local
    private long total = 0L;                                        // expect: field-could-be-local
    private double ratio = 0.0;                                     // expect: field-could-be-local
    private int limit = 10;                                         // a real initial value
    private String label;                                           // expect: field-could-be-local
    private int pack;                                               // expect: field-could-be-local
    private String mode;                                            // expect: field-could-be-local

    void build(List<String> rows) {
        title = "Rows";
        dirty = rows.isEmpty();
        total = rows.size();
        ratio = total / 2.0;
        limit = 3;
        print(title + dirty + total + ratio + limit);
    }

    // `label` here is a parameter: it does not touch the field.
    void rename(String label) {
        print(label);
    }

    void show() {
        label = "x";
        print(label);
    }

    // `pack()` is called on another object inside `pack`: not a recursive call.
    void pack(Packer packer) {
        pack = packer.size();
        packer.pack(pack);
    }

    void describe() {
        mode = "fast";
        print(mode);
    }

    String help() {
        return "the mode flag";                                     // the name in a string
    }

    void print(Object o) { }
}

class Recursive {
    private int depth;

    void walk(int d) {
        depth = d;
        if (depth > 0) {
            this.walk(depth - 1);                                   // really recursive
        }
    }
}

@Slf4j
class Task {
    private String state;                                           // expect: field-could-be-local

    void run() {
        state = "running";
        log.info(state);
    }
}

@Data
class Bean {
    private String state;                                           // read by the generated getter

    void run() {
        state = "running";
        System.out.println(state);
    }
}

class Shared {
    private String name;                                            // used by two methods

    void a() {
        name = "a";
        System.out.println(name);
    }

    void b(Shared other) {
        System.out.println(other.name);                             // the field of another instance
    }
}

class LegacyTest extends junit.framework.TestCase {
    private Service service;                                        // JUnit 3 fixture set-up

    protected void setUp() {
        service = new Service("service");
        service.start();
    }
}
