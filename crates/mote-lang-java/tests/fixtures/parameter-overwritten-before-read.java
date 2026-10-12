import java.io.IOException;
import java.util.List;
import java.util.function.Function;

class Account {
    private String owner;
    private int limit;

    Account(String owner, int limit) {
        owner = "unknown";                                  // expect: parameter-overwritten-before-read
        this.owner = owner;
        this.limit = limit;
    }

    void rename(String name, boolean force) {
        log("renaming");
        name = defaultName();                               // expect: parameter-overwritten-before-read
        owner = name;
    }

    void each(List<String> items) {
        for (String item : items) {
            item = "x";                                     // expect: parameter-overwritten-before-read
            log(item);
        }
        try {
            load();
        } catch (IOException e) {
            e = new IOException("wrapped");                 // expect: parameter-overwritten-before-read
            log(e.getMessage());
        }
        Function<String, String> f = s -> {
            s = "fixed";                                    // expect: parameter-overwritten-before-read
            return s;
        };
    }

    void varargs(String... parts) {
        parts = new String[0];                              // expect: parameter-overwritten-before-read
        log(String.valueOf(parts.length));
    }

    // Negatives.
    String normalize(String name) {
        name = name.trim();                                 // reads the incoming value
        return name;
    }

    void defaults(String name) {
        if (name == null) {
            name = "none";
        }
        log(name);
    }

    void readsFirst(int limit) {
        log("limit " + limit);
        limit = 0;
        log("limit " + limit);
    }

    void nested(String name, boolean b) {
        if (b) {
            name = "x";                                     // conditional: not reported
        }
        log(name);
    }

    void finalParam(final String name) {
        log(name);
    }

    void compound(int count) {
        count += 1;
        log("" + count);
    }

    void fieldTarget(String owner) {
        this.owner = "x";                                   // the field, not the parameter
        log(owner);
    }

    void shadowText(String owner2, String owner) {
        owner2 = owner;                                     // expect: parameter-overwritten-before-read
        log(owner2);
    }

    void compare(int limit) {
        boolean same = limit == 3;
        limit = 4;
        log("" + same + limit);
    }

    Function<String, String> expressionLambda() {
        return s -> s.trim();
    }

    abstract static class Base {
        abstract void run(String name);
    }

    String defaultName() { return "n"; }
    void load() throws IOException {}
    void log(String s) {}
}
