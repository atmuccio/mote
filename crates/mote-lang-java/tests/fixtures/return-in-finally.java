class Finally {
    int positives() {
        try {
            return compute();
        } finally {
            return -1;                                              // expect: return-in-finally
        }
    }

    void throwInFinally() {
        try { work(); } finally {
            if (broken()) {
                throw new IllegalStateException("cleanup failed");  // expect: return-in-finally
            }
        }
    }

    void loops() {
        for (int i = 0; i < 3; i++) {
            try { work(); } finally {
                continue;                                           // expect: return-in-finally
            }
        }
        outer:
        while (true) {
            try { work(); } finally {
                for (int j = 0; j < 2; j++) {
                    break outer;                                    // expect: return-in-finally
                }
            }
        }
        while (true) {
            try { work(); } finally { break; }                      // expect: return-in-finally
        }
    }

    void negatives() {
        try { work(); } finally {
            for (int i = 0; i < 3; i++) {
                if (i == 1) break;
                if (i == 2) continue;
            }
            Runnable r = () -> { return; };
            Object o = new Object() {
                @Override public String toString() { return "x"; }
            };
            try { throw new IllegalStateException(); } catch (IllegalStateException e) { log(e); }
            switch (mode()) { case 1: break; default: break; }
            inner:
            for (;;) { break inner; }
            do { if (done()) break; } while (true);
        }
        try { return; } finally { cleanup(); }
        try { work(); } catch (RuntimeException e) { throw e; } finally { cleanup(); }
    }

    int compute() { return 1; }
}
