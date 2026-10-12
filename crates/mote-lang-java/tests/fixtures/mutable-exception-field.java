import java.io.IOException;

class OrderFailedException extends RuntimeException {
    private static final long serialVersionUID = 1L;
    private static int created;                                    // static: fine
    private final String orderId;
    private String detail;                                         // expect: mutable-exception-field
    int retries, attempts;                                         // expect: mutable-exception-field mutable-exception-field

    OrderFailedException(String orderId) {
        this.orderId = orderId;
    }

    void setDetail(String d) { detail = d; }
}

class BaseError extends Exception {
    protected transient Object payload;                            // expect: mutable-exception-field
}

class SubError extends BaseError {
    String extra;                                                  // expect: mutable-exception-field
}

class IoProblem extends IOException {
    private final int code;
    IoProblem(int code) { this.code = code; }
}

class Oops extends Error {
    Object context;                                                // expect: mutable-exception-field
}

// Superclass from another file: unknown.
class RemoteFailure extends SomeLibraryException {
    String host;
}

// Not an exception.
class Holder extends java.util.ArrayList<String> {
    String label;
}

@lombok.Getter
class LombokError extends RuntimeException {
    private String reason;
}

class Outer extends RuntimeException {
    private final int x = 0;

    static class Inner {
        int notAnException;
    }
}

class Restored extends Exception {
    private transient Object state;

    private void readObject(java.io.ObjectInputStream in) throws java.io.IOException, ClassNotFoundException {
        in.defaultReadObject();
        state = new Object();
    }
}
