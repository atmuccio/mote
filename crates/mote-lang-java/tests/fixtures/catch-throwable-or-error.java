import java.io.IOException;

class CatchAll {
    void positives() {
        try { run(); } catch (Throwable t) { log(t); }                 // expect: catch-throwable-or-error
        try { run(); } catch (Error e) { log(e); }                     // expect: catch-throwable-or-error
        try { run(); } catch (java.lang.Throwable t) { log(t); }       // expect: catch-throwable-or-error
        try {
            run();
        } catch (IOException | Error e) {                              // expect: catch-throwable-or-error
            log(e);
        }
        try (AutoCloseable c = open()) {
            run();
        } catch (final Throwable t) {                                  // expect: catch-throwable-or-error
            recover();
        }
        Runnable r = () -> {
            try { run(); } catch (Throwable t) { }                     // expect: catch-throwable-or-error
        };
    }

    void negatives() {
        try { run(); } catch (Exception e) { log(e); }
        try { run(); } catch (OutOfMemoryError e) { log(e); }         // specific error
        try { run(); } catch (AssertionError e) { log(e); }
        try { run(); } catch (Throwable t) { cleanup(); throw t; }    // rethrown
        try { run(); } catch (Throwable t) { throw new IllegalStateException(t); } // wrapped
        try {
            run();
        } catch (Error e) {
            log(e);
            throw e;
        }
        String text = "catch (Throwable t) { }";
    }
}

class Shadowing {
    static class Error extends Exception {}          // a project class named Error

    void m() {
        try { run(); } catch (Error e) { log(e); }   // not java.lang.Error
    }
}

class Rethrowing {
    void ifElseRethrow() {
        try {
            run();
        } catch (Throwable e) {
            cleanup();
            if (e instanceof RuntimeException) {
                throw (RuntimeException) e;
            } else {
                throw new IllegalStateException(e);
            }
        }
    }

    void ifWithoutElse() {
        try {
            run();
        } catch (Throwable e) {                                        // expect: catch-throwable-or-error
            if (e instanceof RuntimeException) {
                throw (RuntimeException) e;
            }
        }
    }

    void oneBranchLogs() {
        try {
            run();
        } catch (Throwable e) {                                        // expect: catch-throwable-or-error
            if (e instanceof RuntimeException) {
                throw (RuntimeException) e;
            } else {
                log(e);
            }
        }
    }

    void afterErrorClause() {
        try {
            run();
        } catch (Error e) {                                            // expect: catch-throwable-or-error
            log(e);
        } catch (Throwable t) {
            log(t);                                                    // Errors were taken above
        }
    }

    void beforeErrorClause() {
        try {
            run();
        } catch (IllegalStateException e) {
            log(e);
        } catch (Throwable t) {                                        // expect: catch-throwable-or-error
            log(t);
        }
    }
}
