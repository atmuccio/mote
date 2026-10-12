import java.util.List;
import java.util.Map;

class NullCheckAfterDereference {
    private String field;

    int positives(Map<String, String> map, List<String> items, String[] parts, Object lock) {
        String name = map.get("k");
        int length = name.length();
        if (name != null) {                                  // expect: null-check-after-dereference
            length++;
        }
        for (String item : items) {
            use(item);
        }
        if (items == null) {                                 // expect: null-check-after-dereference
            return 0;
        }
        String first = parts[0];
        use(first);
        if (parts == null || parts.length == 0) {            // expect: null-check-after-dereference
            return 1;
        }
        synchronized (lock) {
            use("locked");
        }
        if (null == lock) {                                  // expect: null-check-after-dereference
            return 2;
        }
        return length;
    }

    NullCheckAfterDereference(String value) {
        use(value.trim());
        if (value == null) {                                 // expect: null-check-after-dereference
            throw new IllegalArgumentException();
        }
    }

    void inConditions(String a, String b, String c) {
        if (a.isEmpty()) {
            return;
        }
        if (a == null) {                                     // expect: null-check-after-dereference
            return;
        }
        while (b.length() > 3) {
            b = b.substring(1);
        }
        if (b == null) {
            return;
        }
        for (int i = 0; i < c.length(); i++) {
            use("x");
        }
        if (c != null) {                                     // expect: null-check-after-dereference
            use(c);
        }
    }

    void methodRef(List<String> out, StringBuilder sb) {
        out.forEach(sb::append);
        if (sb != null) {                                    // expect: null-check-after-dereference
            use(sb.toString());
        }
    }

    // ---- compliant ------------------------------------------------------------------------

    void negatives(Map<String, String> map, String a, String b, String c, String d, boolean f) {
        String name = map.get("k");
        if (name != null) {
            use(name.trim());
        }
        String other = map.get("x");
        other = other.trim();
        if (other != null) {
            use(other);
        }
        if (f && a.isEmpty()) {
            use("maybe");
        }
        if (a != null) {
            use(a);
        }
        use(f ? b.trim() : "");
        if (b == null) {
            return;
        }
        Runnable r = () -> use(c.trim());
        if (c == null) {
            return;
        }
        try {
            use(d.trim());
        } catch (RuntimeException e) {
            use("ignored");
        }
        if (d == null) {
            return;
        }
        use(field.trim());
        if (field == null) {
            return;
        }
    }

    void loops(List<String> items, String e) {
        for (String s : items) {
            use(e.trim());
        }
        if (e != null) {
            use(e);
        }
    }

    void passAsArgument(String a, String b) {
        use(a);
        if (a == null) {
            return;
        }
        assert b.length() > 0;
        if (b == null) {
            return;
        }
    }

    void reassignedInBetween(Map<String, String> map) {
        String v = map.get("a");
        use(v.trim());
        v = map.get("b");
        if (v == null) {
            return;
        }
    }

    void shadowedInLambda(List<String> items, String s) {
        items.forEach(x -> use(x.trim()));
        if (s == null) {
            return;
        }
    }

    void anonymous(String s) {
        Object o = new Object() {
            @Override
            public String toString() {
                return s.trim();
            }
        };
        if (s == null) {
            return;
        }
    }

    void use(Object o) {}
}
