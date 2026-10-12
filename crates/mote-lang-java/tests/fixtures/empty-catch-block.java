import java.io.IOException;

class EmptyCatchDemo {
    void positives() {
        try { run(); } catch (Exception e) { }                  // expect: empty-catch-block
        try {
            run();
        // expect-next-line: empty-catch-block
        } catch (IOException | RuntimeException e) {
        }
        try (AutoCloseable c = open()) {
            run();
        } catch (Exception ex) {}                               // expect: empty-catch-block
        Runnable r = () -> {
            try { run(); } catch (IllegalStateException e) {}   // expect: empty-catch-block
        };
        new Thread() {
            @Override public void run() {
                try { work(); } catch (Exception e) { }         // expect: empty-catch-block
            }
        };
    }

    void negatives() {
        try { run(); } catch (Exception e) { log(e); }
        try { run(); } catch (Exception e) {
            // deliberately ignored: the cache is optional
        }
        try { run(); } catch (Exception e) { /* best effort */ }
        try { run(); } catch (Exception ignored) { }
        try { run(); } catch (Exception expected) { }
        try { run(); } catch (Exception _) { }
        try { run(); } finally { }
        try { run(); } catch (Exception e) { throw new RuntimeException(e); }
        try { run(); } catch (Exception e) { ; }                 // expect: empty-catch-block
        String text = """
            try { } catch (Exception e) { }
            """;
    }
}
