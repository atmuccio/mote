import java.io.IOException;
import java.io.IOError;
import com.acme.errors.ValidationException;
import com.acme.errors.Widget;
import com.acme.monitor.StartupError;
import org.opentest4j.AssertionFailedError;

class Validator {
    void check(int id, String name) throws IOException {
        if (id < 0) {
            new IllegalArgumentException("negative id: " + id);   // expect: exception-not-thrown
        }
        if (name == null) {
            new NullPointerException();                             // expect: exception-not-thrown
        }
        new IOException("disk");                                    // expect: exception-not-thrown
        new ValidationException(name);                              // expect: exception-not-thrown
        new com.acme.OtherException();                              // expect: exception-not-thrown
        new LocalFailure();                                         // expect: exception-not-thrown
        Runnable r = () -> {
            new IllegalStateException("lambda");                    // expect: exception-not-thrown
        };
        // Errors: JDK ones (resolved, imported, qualified, or java.lang by simple name), test
        // assertion errors, and this file's own Error subclasses.
        new AssertionError("unreachable");                          // expect: exception-not-thrown
        new InternalError("state");                                 // expect: exception-not-thrown
        new java.lang.NoSuchMethodError("m");                       // expect: exception-not-thrown
        new IOError(null);                                          // expect: exception-not-thrown
        new AssertionFailedError("diff");                           // expect: exception-not-thrown
        new LocalError();                                           // expect: exception-not-thrown

        // Compliant
        if (id == 0) {
            throw new IllegalArgumentException("zero");
        }
        RuntimeException saved = new RuntimeException("kept");
        record(new IllegalStateException("passed on"));
        new Widget(name);                                           // not an exception by name
        new NotAnError();                                           // proven not Throwable
        new Exception() { };                                        // anonymous subclass
        new Thread(() -> { }).start();
        // `…Error` classes the resolver cannot see are often not exceptions: a monitor that
        // registers itself when constructed, an error-report object.
        new StartupError("db", "Database unavailable", null);      // imported, not a JDK error
        new com.acme.OtherError();                                  // qualified, not a JDK error
        new ReportError("same package, unknown");                   // unresolved simple name
        new UnknownBaseError();                                     // local, unknown supertype
    }

    void record(Exception e) { }

    static class LocalFailure extends RuntimeException { }

    static class LocalError extends Error { }

    static class UnknownBaseError extends com.acme.Monitor { }

    static class NotAnError { }

    static class NotAnErrorError extends Object { }

    void provenNot() {
        new NotAnErrorError();                                      // proven not Throwable
    }
}
