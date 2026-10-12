class Name implements Cloneable {
    private String value;

    @Override
    public String toString() {
        if (value == null) {
            return null;                                        // expect: tostring-or-clone-returns-null
        }
        return value;
    }

    @Override
    public Name clone() {
        try {
            return (Name) super.clone();
        } catch (CloneNotSupportedException e) {
            return null;                                        // expect: tostring-or-clone-returns-null
        }
    }
}

class Label {
    private String text;

    @Override
    public String toString() {
        return text == null ? null : text.trim();               // expect: tostring-or-clone-returns-null
    }
}

class Fine {
    private String value;

    @Override
    public String toString() {
        java.util.function.Supplier<String> s = () -> null;
        Object o = new Object() {
            @Override
            public String toString() { return "inner"; }
        };
        return value == null ? "" : value;
    }

    public String toString(int indent) {
        return null;
    }

    static String clone(String s) {
        return null;
    }

    String describe() {
        return null;
    }
}
