import java.io.IOException;

class Thrower {
    void positives(int port) throws Exception {
        if (port < 0) throw new RuntimeException("negative");         // expect: generic-exception-thrown
        if (port == 0) throw new Exception("zero");                   // expect: generic-exception-thrown
        if (port == 1) throw new Throwable();                         // expect: generic-exception-thrown
        if (port == 2) throw new Error("two");                        // expect: generic-exception-thrown
        if (port == 3) throw new java.lang.RuntimeException();        // expect: generic-exception-thrown
        try { io(); } catch (IOException e) {
            throw new RuntimeException(e);                            // expect: generic-exception-thrown
        }
        Runnable r = () -> { throw new RuntimeException("lambda"); }; // expect: generic-exception-thrown
    }

    void negatives(int port) throws Exception {
        if (port < 0) throw new IllegalArgumentException("negative");
        if (port == 0) throw new IOException("zero");
        if (port == 1) throw new AssertionError();
        RuntimeException saved = new RuntimeException("kept");        // not thrown here
        throw saved;
    }

    void more() {
        throw new UncheckedIOException(null);
    }
}

class Custom {
    static class RuntimeException extends java.lang.IllegalStateException {}
    void m() { throw new RuntimeException(); }                       // project type
}
