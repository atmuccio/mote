class LoopNeverEntered {
    int field;

    void positives(int[] a) {
        for (int i = 10; i < 5; i++) {                       // expect: loop-never-entered
            use(i);
        }
        for (int i = 0; i > 0; i--) {                        // expect: loop-never-entered
            use(i);
        }
        for (int i = 0, n = 0; i < n; i++) {                 // expect: loop-never-entered
            use(i);
        }
        int k;
        for (k = 3; k <= 2; k++) {                           // expect: loop-never-entered
            use(k);
        }
        int remaining = 0;
        while (remaining > 0) {                              // expect: loop-never-entered
            remaining--;
        }
        boolean done = true;
        while (!done) {                                      // expect: loop-never-entered
            done = step();
        }
        long x = -1L;
        while (x >= 0 && step()) {                           // expect: loop-never-entered
            x--;
        }
    }

    void negatives(int[] a, int n) {
        for (int i = 0; i < 5; i++) {
            use(i);
        }
        for (int i = 10; i > 5; i--) {
            use(i);
        }
        for (int i = 0; i < n; i++) {
            use(i);
        }
        for (int i = 0; i < a.length; i++) {
            use(i);
        }
        int remaining = 3;
        while (remaining > 0) {
            remaining--;
        }
        boolean done = false;
        while (!done) {
            done = step();
        }
        int y = 0;
        y = n;
        while (y > 0) {
            y--;
        }
        boolean more = false;
        do {
            more = step();
        } while (more);
        for (;;) {
            if (step()) break;
        }
        char c = 'z';
        while (c < 'a') {
            c++;
        }
        int z = 0;
        while (z > 0 || step()) {
            z++;
        }
        for (int i = 0; field > i; i++) {
            use(i);
        }
    }

    boolean step() {
        return true;
    }

    void use(long i) {}
}
