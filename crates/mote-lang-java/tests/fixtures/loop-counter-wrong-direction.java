import java.util.List;

class Loops {
    void wrong(int[] items, List<String> names, int n) {
        for (int i = items.length - 1; i < items.length; i--) {            // expect: loop-counter-wrong-direction
            use(items[i]);
        }
        for (int i = 0; i >= 0 && i < n; i++) { }
        for (int i = n; i >= 0; i++) {                                       // expect: loop-counter-wrong-direction
            use(i);
        }
        for (int i = 0; i < names.size(); i -= 1) {                          // expect: loop-counter-wrong-direction
            use(i);
        }
        for (int i = 10; 0 < i; i++) {                                       // expect: loop-counter-wrong-direction
            use(i);
        }
        for (char c = 'z'; c >= 'a'; c++) {                                  // expect: loop-counter-wrong-direction
            use(c);
        }
        for (int i = n; i > 0; i = i + 1) {                                  // expect: loop-counter-wrong-direction
            use(i);
        }
        for (int i = 0, j = n; i < n; i++, j--) {
            use(j);
        }
    }

    void fine(int[] items, int n, int step) {
        for (int i = 0; i < items.length; i++) { use(i); }
        for (int i = items.length - 1; i >= 0; i--) { use(i); }
        for (int i = n; i > 0; i -= 2) { use(i); }
        for (int i = 0; i < n; i += step) { use(i); }          // unknown step sign
        for (int i = 0; i < n; i -= step) { use(i); }          // unknown step sign
        for (int i = 0; i != n; i--) { use(i); }               // != is not checked
        for (int i = 0; i < n; ) { i++; }
        for (int i = 0; i < n; i--) { i += 2; }               // body also moves the counter
        for (int i = 0; i < n + i; i--) { use(i); }            // bound mentions the counter
        for (int i = 0, j = n; i < j; i++, j--) { use(i); }   // both move towards each other
        for (int i = 0, j = n; i < j; i++, j++) { use(i); }   // the bound moves too: not judged
        for (int i = 0; i < limit(); i++) { use(i); }
        for (;;) { break; }
        Runnable r = () -> {
            for (int k = 0; k < 3; k++) { use(k); }
        };
    }

    int limit() { return 3; }

    void use(int i) { }
}
