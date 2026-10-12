class Flags {
    boolean positives(boolean ready, boolean armed) {
        boolean a = ready && true;                                  // expect: boolean-literal-comparison
        boolean b = true && ready;                                  // expect: boolean-literal-comparison
        boolean c = armed || false;                                 // expect: boolean-literal-comparison
        boolean d = (false) || armed;                               // expect: boolean-literal-comparison
        boolean e = !false;                                         // expect: boolean-literal-comparison
        boolean f = !(true);                                        // expect: boolean-literal-comparison
        return a && b && c && d && e && f;
    }

    boolean negatives(boolean ready, boolean armed, Boolean boxed) {
        boolean a = ready && false;                                 // constant condition: another problem
        boolean b = armed || true;
        boolean c = ready && armed;
        boolean d = !ready;
        boolean e = !Boolean.TRUE;
        int bits = 5 & 3;
        return a || b || c || d || e || bits > 0;
    }
}
