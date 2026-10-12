import java.io.IOException;

class Causes {
    int positives(String text) {
        try {
            return Integer.parseInt(text);
        } catch (NumberFormatException e) {
            throw new IllegalArgumentException("invalid: " + text);          // expect: lost-exception-cause
        }
    }

    void more() {
        try { io(); } catch (IOException | RuntimeException e) {
            throw new IllegalStateException();                              // expect: lost-exception-cause
        }
        try { io(); } catch (IOException e) {
            log(e);
            throw new ServiceException(                                     // expect: lost-exception-cause
                "failed");
        }
        try { io(); } catch (IOException e) {
            if (retry) {
                throw new ServiceException("retry failed", e);
            } else {
                throw new ServiceException("gave up");                      // expect: lost-exception-cause
            }
        }
        try (AutoCloseable c = open()) {
            use(c);
        } catch (Exception e) {
            throw (new IllegalStateException("closing"));                   // expect: lost-exception-cause
        }
        try { io(); } catch (IOException outer) {
            try { io(); } catch (IOException inner) {
                throw new ServiceException("inner");                        // expect: lost-exception-cause
            }
        }
        try { io(); } catch (java.util.concurrent.ExecutionException e) {
            Throwable c = e.getCause();
            String late = "x";
            throw new ServiceException(late);                               // expect: lost-exception-cause
        }
    }

    void derived() {
        try { io(); } catch (IOException outer) {
            try { io(); } catch (IOException inner) {
                log(inner);
                throw new ServiceException("retry failed", outer);         // chains the original failure
            }
        }
        try { io(); } catch (java.util.concurrent.ExecutionException e) {
            Throwable c = e.getCause();
            if (c instanceof RuntimeException r) {
                throw new IllegalStateException(r);
            }
            throw new IllegalStateException(c);                             // unwrapped cause
        }
        try { io(); } catch (java.util.concurrent.ExecutionException e) {
            Throwable root;
            root = e.getCause();
            throw new IllegalStateException(root);
        }
    }

    void negatives() throws Exception {
        try { io(); } catch (IOException e) { throw new ServiceException("x", e); }
        try { io(); } catch (IOException e) { throw new ServiceException(e); }
        try { io(); } catch (IOException e) { throw e; }
        try { io(); } catch (IOException e) { throw wrap(e); }
        try { io(); } catch (IOException e) {
            ServiceException se = new ServiceException("x");
            se.initCause(e);
            throw se;
        }
        try { io(); } catch (IOException e) {
            IllegalStateException ise = new IllegalStateException("y");
            ise.addSuppressed(e);
            throw new IllegalStateException("z");
        }
        try { io(); } catch (IOException e) {
            throw (ServiceException) new ServiceException("x").initCause(e);
        }
        try { io(); } catch (NumberFormatException ignored) {
            throw new IllegalArgumentException("not a number");
        }
        try { io(); } catch (IOException e) {
            Runnable r = () -> { throw new IllegalStateException("later"); };
            run(r, e);
        }
        try { io(); } catch (IOException e) {
            try {
                throw new ServiceException("probe");
            } catch (ServiceException probe) {
                log(probe, e);
            }
        }
        try { io(); } catch (IOException e) {
            new Thread() {
                public void run() { throw new IllegalStateException("elsewhere"); }
            }.start();
        }
        try { io(); } finally {
            throw new IllegalStateException("no catch here");
        }
    }

    void messageOnly(String key) {
        try { io(); } catch (IOException e) {
            throw new ServiceException(e.getMessage());                     // expect: lost-exception-cause
        }
        try { io(); } catch (IOException e) {
            throw new ServiceException("m: " + e);                          // expect: lost-exception-cause
        }
        try { io(); } catch (IOException e) {
            throw new ServiceException(e.toString());                       // expect: lost-exception-cause
        }
        try { io(); } catch (IOException e) {
            throw new ServiceException(Status.FAILED, key, e.getLocalizedMessage()); // expect: lost-exception-cause
        }
        try { io(); } catch (IOException e) {
            String msg = "cannot read " + key + ": " + e.getMessage();
            throw new ServiceException(msg);                                // expect: lost-exception-cause
        }
        try { io(); } catch (IOException e) {
            throw new ServiceException(e.getClass().getSimpleName() + " while reading"); // expect: lost-exception-cause
        }
    }

    void helpers() {
        try { io(); } catch (IOException e) {
            throw invalidInput("bad input: " + e.getMessage());             // expect: lost-exception-cause
        }
        try { io(); } catch (IOException e) {
            throw Errors.notFound();                                        // expect: lost-exception-cause
        }
        try { io(); } catch (IOException e) {
            throw wrap(e);
        }
        try { io(); } catch (java.util.concurrent.ExecutionException e) {
            throw e.getCause();
        }
        try { io(); } catch (IOException e) {
            this.failure = e;
            throw failure();
        }
        try { io(); } catch (IOException e) {
            throw Errors.translate(key(), e);
        }
    }

    void chainedWithText() {
        try { io(); } catch (IOException e) {
            throw new ServiceException(e.getMessage(), e);
        }
        try { io(); } catch (IOException e) {
            throw new ServiceException(ExceptionUtils.getStackTrace(e));
        }
        try { io(); } catch (IOException e) {
            throw new ServiceException(e.getCause());
        }
        try { io(); } catch (IOException e) {
            throw new ServiceException(String.valueOf(e));
        }
        try { io(); } catch (IOException e) {
            String trace = stackTraceOf(e);
            throw new ServiceException(trace);
        }
        try { io(); } catch (IOException e) {
            throw new ServiceException(() -> e.getMessage());
        }
        try { io(); } catch (IOException e) {
            Object detail = e.getMessage();
            throw new ServiceException(detail);
        }
    }

    void notInCatch() {
        throw new IllegalStateException("plain");
    }
}
