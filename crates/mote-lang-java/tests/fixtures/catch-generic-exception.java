import java.io.IOException;
import java.sql.SQLException;

class Generic {
    void positives() {
        try { run(); } catch (Exception e) { log(e); }                     // expect: catch-generic-exception
        try { run(); } catch (RuntimeException e) { log(e); }              // expect: catch-generic-exception
        try { run(); } catch (java.lang.Exception e) { log(e); }           // expect: catch-generic-exception
        try {
            run();
        } catch (IOException e) {
            retry();
        } catch (Exception e) {                                            // expect: catch-generic-exception
            log(e);
        }
        try (java.io.InputStream c = open()) {
            use(c);
        } catch (Exception e) {                                            // expect: catch-generic-exception
            fallback();
        }
        new Thread() {
            @Override public void run() {
                try { work(); } catch (RuntimeException e) { log(e); }    // expect: catch-generic-exception
            }
        };
    }

    void negatives() {
        try { run(); } catch (IOException | SQLException e) { log(e); }
        try { run(); } catch (IllegalArgumentException e) { log(e); }
        try { run(); } catch (Throwable t) { log(t); }                      // other rule
        try { run(); } catch (Exception e) { throw new IllegalStateException("wrapped", e); }
        try {
            run();
        } catch (Exception e) {
            log(e);
            throw e;
        }
        try { run(); } catch (Unknown e) { log(e); }
        try { run(); } catch (Exception e) { }                              // left to empty-catch-block
    }

    // Callees declared `throws Exception` leave nothing narrower to catch.
    void forced(Object o, java.util.concurrent.Callable<String> task) {
        if (o instanceof AutoCloseable closeable) {
            try {
                closeable.close();
            } catch (Exception e) {
                log(e);
            }
        }
        try {
            task.call();
        } catch (Exception e) {
            log(e);
        }
        try (AutoCloseable c = open()) {
            use(c);
        } catch (Exception e) {
            log(e);
        }
        try {
            task.call();
        } catch (RuntimeException e) {                                     // expect: catch-generic-exception
            log(e);
        } catch (Exception e) {
            log(e);
        }
    }
}

class Local {
    static class Exception extends java.lang.RuntimeException {}   // shadows java.lang.Exception
    void m() {
        try { run(); } catch (Exception e) { log(e); }                // project type
    }
}
