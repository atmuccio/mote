import java.io.IOException;

class Useless {
    void positives() throws Exception {
        try { run(); } catch (IOException e) { throw e; }          // expect: useless-catch
        try {
            run();
        } catch (final RuntimeException ex) {                     // expect: useless-catch
            throw ex;
        }
        try {
            run();
        } catch (IllegalStateException e) {
            recover();
        } catch (IOException e) {                                  // expect: useless-catch
            // just pass it on
            throw e;
        } finally {
            cleanup();
        }
        try (AutoCloseable c = open()) {
            run();
        } catch (Exception e) { throw (e); }                        // expect: useless-catch
    }

    void negatives() throws Exception {
        // A rethrow that shields a broader handler below is meaningful.
        try {
            run();
        } catch (IOException e) {
            throw e;
        } catch (Exception e) {
            log(e);
        }
        try { run(); } catch (IOException e) { log(e); throw e; }
        try { run(); } catch (IOException e) { throw new IllegalStateException(e); }
        try { run(); } catch (IOException e) { throw other; }
        try {
            run();
        } catch (IOException e) {
            throw e;
        }
        // comment between the clauses
        catch (Exception e) {
            log(e);
        }
        Runnable r = () -> {
            try { run(); } catch (RuntimeException e) { log(e); }
        };
    }
}
