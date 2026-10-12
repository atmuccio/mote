import java.io.IOException;
import java.sql.SQLException;
import java.util.concurrent.ExecutionException;
import java.util.concurrent.TimeoutException;

class Wrapping extends Base {
    Object get(java.util.concurrent.Future<Object> f) {
        try {
            return f.get();
        } catch (TimeoutException ex) {
            throw wrap(ex);
        } catch (ExecutionException ex) {
            throw wrap(ex);                                         // each clause picks its overload
        } catch (InterruptedException ex) {
            throw new IllegalStateException(ex);
        }
    }

    static RuntimeException wrap(Exception ex) { return new RuntimeException(ex); }

    static RuntimeException wrap(ExecutionException ex) { return new RuntimeException(ex.getCause()); }

    void inherited() {
        try {
            read();
        } catch (IOException e) {
            handle(e);
        } catch (SQLException e) {
            handle(e);                                              // declared elsewhere: overloads unknown
        }
    }

    void singleOverload() {
        try {
            read();
        } catch (IOException e) {
            report(e);
        } catch (SQLException e) {                                  // expect: identical-catch-branches
            report(e);
        }
    }

    void constructorOverloads() {
        try {
            read();
        } catch (IOException e) {
            throw new Failure(e);
        } catch (SQLException e) {
            throw new Failure(e);                                   // Failure(IOException) vs Failure(Exception)
        }
    }

    void externalCall(org.slf4j.Logger log) {
        try {
            read();
        } catch (IOException e) {
            log.warn("failed", e);
        } catch (SQLException e) {                                  // expect: identical-catch-branches
            log.warn("failed", e);
        }
    }

    void notPassed() {
        try {
            read();
        } catch (IOException e) {
            handle(null);
        } catch (SQLException e) {                                  // expect: identical-catch-branches
            handle(null);
        }
    }

    void report(Exception e) { }

    void read() throws IOException, SQLException { }

    static class Failure extends RuntimeException {
        Failure(IOException e) { super(e); }

        Failure(Exception e) { super(e); }
    }
}
