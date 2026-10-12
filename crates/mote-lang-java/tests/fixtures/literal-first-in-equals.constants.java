import com.acme.Names;

interface Keys {
    String KIND = "kind";
}

class Constants implements Keys {
    static final String ADMIN = "admin";
    private static final String PREFIX = "role" + ":" + "x";
    static String mutable = "m";
    final String instance = "i";
    static final String COMPUTED = System.getProperty("user");
    static final Object ANY = "any";
    static final String[] LIST = {"a"};

    static class Nested {
        static final String GUEST = "guest";
        static final String DYNAMIC = String.valueOf(1);
    }

    boolean check(String role, String other) {
        boolean a = role.equals(ADMIN);                         // expect: literal-first-in-equals
        boolean b = other.equalsIgnoreCase(PREFIX);             // expect: literal-first-in-equals
        boolean c = lookup(role).equals(Nested.GUEST);          // expect: literal-first-in-equals
        boolean d = role.equals(KIND);                          // expect: literal-first-in-equals

        boolean ok1 = ADMIN.equals(role);
        boolean ok2 = role.equals(mutable);                     // not final: may change
        boolean ok3 = role.equals(instance);                    // instance field
        boolean ok4 = role.equals(COMPUTED);                    // value not a literal: may be null
        boolean ok5 = role.equals(ANY);                         // declared Object
        boolean ok6 = role.equals(Nested.DYNAMIC);
        boolean ok7 = role.equals(Names.ADMIN);                 // declared in another file
        final String local = "x";
        boolean ok8 = role.equals(local);                       // a local, not a constant field
        boolean ok9 = other != null && other.equals(ADMIN);     // checked just before
        boolean ok10 = role.trim().equals(ADMIN);
        boolean ok11 = lookup(role).equals(Names.TYPE);         // other file: off by default
        return a && b && c && d;
    }

    String lookup(String key) {
        return null;
    }

    boolean shadowed(String role) {
        String ADMIN = null;
        return role.equals(ADMIN);                              // the local, not the constant
    }
}
