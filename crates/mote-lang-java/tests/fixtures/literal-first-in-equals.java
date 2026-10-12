import java.util.Map;

class Roles {
    private static final String ADMIN = "admin";
    private String current;

    boolean check(String role, Map<String, String> attrs, Object any, Unknown u, StringBuilder sb) {
        boolean a = role.equals("admin");                           // expect: literal-first-in-equals
        boolean b = current.equalsIgnoreCase("ROOT");               // expect: literal-first-in-equals
        boolean c = attrs.get("kind").equals("user");               // expect: literal-first-in-equals
        boolean d = this.current.equals("x");                       // expect: literal-first-in-equals

        boolean ok1 = "admin".equals(role);
        boolean ok2 = role.trim().equals("admin");                  // trim() already needs non-null
        boolean ok3 = role.toLowerCase().equals("admin");
        boolean ok4 = ADMIN.equals("admin");                        // constant receiver
        boolean ok5 = any.equals("admin");                          // not a String
        boolean ok6 = u.equals("admin");                            // declared with another type
        boolean e = role.equals(ADMIN);                             // expect: literal-first-in-equals
        boolean ok8 = (role + "x").equals("adminx");
        boolean ok9 = String.valueOf(any).equals("null");
        boolean ok10 = role.equals("a", "b");                       // other arity
        boolean ok11 = sb.equals("x");
        boolean ok12 = role != null && role.equals("admin");        // checked just before
        boolean ok13 = role == null || role.equalsIgnoreCase("x");
        boolean ok14 = StringUtils.isEmpty(role) || role.equals("x");
        if (current != null) {
            boolean ok15 = current.equals("root");
        }
        String ok16 = role != null ? (role.equals("a") ? "A" : "?") : "";
        if (role.length() > 3) {
            boolean ok17 = role.equals("long");                     // dereferenced by the condition
        }
        if (current.equals("a")) {                                  // expect: literal-first-in-equals
            a = false;
        } else if (current.equals("b")) {                           // the chain already dereferenced it
            a = true;
        }
        boolean ok18 = current.equals("x") || current.equals("y");  // expect: literal-first-in-equals
        return a && b && c && d;
    }
}

class NonNullLocals {
    void trimmed(String[] keys) {
        for (String key : keys) {
            String trimmed = key.trim();
            if (trimmed.equals("|")) {
                continue;
            }
            String type = key.substring(1);
            if (type.equals("B") || type.equals("H")) {
                type = "I";
            }
        }
    }

    void reassignedToNull(String key) {
        String t = key.trim();
        if (t.isEmpty()) {
            t = null;
        }
        if (t.equals("x")) {                            // expect: literal-first-in-equals
            return;
        }
    }

    void reassignedFromUnknown(String key, java.util.Map<String, String> m) {
        String t = key.trim();
        t = m.get(t);
        if (t.equals("x")) {                            // expect: literal-first-in-equals
            return;
        }
    }

    void initializedFromUnknown(java.util.Map<String, String> m) {
        String t = m.get("k");
        if (t.equals("x")) {                            // expect: literal-first-in-equals
            return;
        }
    }

    void chained(String key) {
        String a = key.toLowerCase();
        String b = a;
        if (b.equals("y")) {
            return;
        }
    }

    public static void main(String[] args) {
        if (args[0].equalsIgnoreCase("start")) {
            return;
        }
    }

    static void notMain(String[] args) {
        if (args[0].equals("start")) {                  // expect: literal-first-in-equals
            return;
        }
    }
}
