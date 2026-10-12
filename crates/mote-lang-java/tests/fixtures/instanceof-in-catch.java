import java.util.concurrent.ExecutionException;
import java.util.concurrent.TimeoutException;

class Sync {
    void run() {
        try {
            sync();
        } catch (Exception e) {
            if (e instanceof TimeoutException) {                    // expect: instanceof-in-catch
                retryLater();
            } else if (e instanceof RuntimeException re) {          // expect: instanceof-in-catch
                throw re;
            }
        }

        try {
            sync();
        } catch (ExecutionException e) {
            if (e.getCause() instanceof TimeoutException) {         // the cause, not the exception
                retryLater();
            }
        }

        try {
            sync();
        } catch (Exception e) {
            Object other = e;
            if (other instanceof String) {                          // another variable
                retryLater();
            }
            boolean b = (e) instanceof IllegalStateException;       // expect: instanceof-in-catch
        }
    }

    void unwrapped() {
        try {
            sync();
        } catch (Exception ex) {
            if (ex instanceof ExecutionException) {                 // expect: instanceof-in-catch
                Throwable cause = ex.getCause();
                if (cause instanceof Exception) {
                    ex = (Exception) cause;
                }
            }
            if (ex instanceof TimeoutException) {                   // the unwrapped cause
                retryLater();
            }
        }
    }

    void sync() throws Exception { }

    void retryLater() { }
}
