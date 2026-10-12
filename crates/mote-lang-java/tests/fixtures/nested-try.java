import java.io.IOException;

class Loader {
    void load(String path) {
        try {
            read(path);
            try {                                                  // expect: nested-try
                connect();
            } catch (java.io.FileNotFoundException e) {
                offline();
            }
        } catch (IOException e) {
            report(e);
        }
    }

    void unrelated(String path) {
        try {
            read(path);
            try {
                connect();
            } catch (IllegalStateException e) {
                offline();
            }
        } catch (IOException e) {
            report(e);
        }
    }

    void catchAll(String path) {
        try {
            try {                                                  // expect: nested-try
                read(path);
            } catch (Exception e) {
                offline();
            }
        } catch (MyAppException e) {
            report(e);
        }
        try {
            try {                                                  // expect: nested-try
                read(path);
            } catch (MyAppException | IllegalStateException e) {
                offline();
            }
        } catch (MyAppException e) {
            report(e);
        }
        try {
            try {
                read(path);
            } catch (MyAppException e) {
                offline();
            }
        } catch (OtherAppException e) {
            report(e);
        }
    }

    void resources(String path) {
        try (java.io.InputStream in = open(path)) {
            try (java.io.InputStream other = open(path)) {         // expect: nested-try
                other.read();
            } catch (IOException e) {
                offline();
            }
        } catch (IOException e) {
            report(e);
        }
    }

    void fine(String path) throws IOException {
        // Outer try-finally: cleanup, fine.
        try {
            try {
                connect();
            } catch (IllegalStateException e) {
                offline();
            }
        } finally {
            offline();
        }
        // Inner try-finally: cleanup, fine.
        try {
            try {
                connect();
            } finally {
                offline();
            }
        } catch (IllegalStateException e) {
            offline();
        }
        // In a catch or finally block, a loop or an if: fine.
        try {
            connect();
        } catch (IllegalStateException e) {
            try {
                offline();
            } catch (RuntimeException e2) {
                report(null);
            }
        }
        try {
            for (int i = 0; i < 3; i++) {
                try {
                    connect();
                } catch (IllegalStateException e) {
                    offline();
                }
            }
        } catch (RuntimeException e) {
            offline();
        }
    }

    void read(String p) throws IOException { }
    java.io.InputStream open(String p) throws IOException { return null; }
    void connect() { }
    void offline() { }
    void report(Exception e) { }
}
