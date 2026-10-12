class BoolLiterals {
    private Boolean cached;

    boolean positives(boolean valid, boolean enabled, int count) {
        if (valid == true) { go(); }                    // expect: boolean-literal-comparison
        if (enabled != false) { go(); }                 // expect: boolean-literal-comparison
        if (false == isReady()) { go(); }               // expect: boolean-literal-comparison
        if (cached == Boolean.TRUE || cached != true) { go(); }  // expect: boolean-literal-comparison
        boolean a = count > 0 ? true : false;           // expect: boolean-literal-comparison
        boolean b = valid ? false : true;               // expect: boolean-literal-comparison
        return (a == (false)) && b;                     // expect: boolean-literal-comparison
    }

    boolean negatives(boolean valid, Object o) {
        boolean a = valid ? true : isReady();
        boolean b = valid ? true : true;                // identical branches: another rule
        boolean c = true && valid;                      // expect: boolean-literal-comparison
        boolean d = valid;
        boolean e = Boolean.TRUE.equals(cached);
        boolean f = true == false;                      // two literals
        return a && b && c && d && e && f;
    }

    boolean isReady() { return true; }
    void go() {}
}
