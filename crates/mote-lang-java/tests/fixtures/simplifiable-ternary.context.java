class Flags {
    private boolean enabled;
    private Boolean boxed;
    private Object any;

    boolean visible(Item item, boolean admin) {
        boolean a = admin ? true : item.isPublic();                 // expect: simplifiable-ternary
        enabled = admin ? item.isOpen() : false;                    // expect: simplifiable-ternary
        if (admin && (item.isLocked() ? false : item.isOpen())) {   // expect: simplifiable-ternary
            return true;
        }
        boolean b = !(admin ? item.flag() : true);                  // expect: simplifiable-ternary
        return admin ? true : item.isPublic();                      // expect: simplifiable-ternary
    }

    Object traps(Item item, boolean admin) {
        Object o = admin ? true : item.value();                     // Object context: the branch may be anything
        Boolean b = admin ? true : item.maybe();                    // Boolean context: not reported without types
        any = admin ? false : item.value();
        String s = "" + (admin ? true : item.value());
        take(admin ? true : item.value());                          // parameter type unknown here
        Runnable r = () -> check(admin ? true : item.value());
        return admin ? true : item.value();                         // the method returns Object
    }

    boolean[] arrays(Item item, boolean admin) {
        boolean[] flags = { admin ? true : item.isOpen() };
        return null;
    }

    void take(Object o) { }
    void check(boolean b) { }
}
