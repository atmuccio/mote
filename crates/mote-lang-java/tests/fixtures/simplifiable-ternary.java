class Visibility {
    boolean visible(boolean isAdmin, Item item, int count, Boolean flag, Object o, Unknown u) {
        boolean a = isAdmin ? true : item.size() > 0;               // expect: simplifiable-ternary
        boolean b = isAdmin ? false : count > 3;                    // expect: simplifiable-ternary
        boolean c = count > 0 ? !isAdmin : true;                    // expect: simplifiable-ternary
        boolean d = isAdmin ? flag : false;                         // expect: simplifiable-ternary
        boolean e = isAdmin ? (o instanceof String) : false;        // expect: simplifiable-ternary

        boolean ok1 = isAdmin ? true : false;                       // java/boolean-literal-comparison
        Object ok2 = isAdmin ? true : null;
        Object ok3 = isAdmin ? true : o;
        boolean f = isAdmin ? u.check() : false;                    // expect: simplifiable-ternary
        String ok5 = isAdmin ? "yes" : "no";
        return a;
    }
}
