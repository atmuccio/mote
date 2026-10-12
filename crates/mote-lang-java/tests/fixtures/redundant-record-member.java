import java.util.List;

record Point(int x, int y) {
    private static final long serialVersionUID = 1L;               // expect: redundant-record-member

    Point(int x, int y) {                                          // expect: redundant-record-member
        this.x = x;
        this.y = y;
    }

    public int x() {                                               // expect: redundant-record-member
        return x;
    }

    @Override
    public int y() {                                               // expect: redundant-record-member
        return this.y;
    }
}

record Empty(String name) {
    Empty { }                                                      // expect: redundant-record-member
}

record Validating(String name, List<String> tags) {
    Validating {
        if (name == null) throw new IllegalArgumentException();
        tags = List.copyOf(tags);
    }

    // Copies: fine.
    public List<String> tags() {
        return List.copyOf(tags);
    }

    // Normalizes: fine.
    public String name() {
        return name.trim();
    }

    // Not an accessor: fine.
    public String name(String prefix) {
        return prefix + name;
    }
}

record Annotated(String name, int age) {
    @JsonCreator
    Annotated(String name, int age) {
        this.name = name;
        this.age = age;
    }

    @JsonProperty("n")
    public String name() {
        return name;
    }

    /** The age in years. */
    public int age() {
        return age;
    }
}

record Documented(String id) {
    Documented {
        // nothing to check yet
    }
}

record Partial(int a, int b) {
    Partial(int a, int b) {
        this.a = a;
        this.b = Math.abs(b);
    }
}

record Reordered(int a, int b) {
    Reordered(int b, int a) {
        this.a = a;
        this.b = b;
    }
}

class NotARecord {
    private static final long serialVersionUID = 1L;
    int x;

    public int x() {
        return x;
    }
}
