import java.io.IOException;

class Printing {
    void positives() {
        try { io(); } catch (IOException e) {
            e.printStackTrace();                            // expect: print-stack-trace
        }
        try { io(); } catch (IOException | RuntimeException e) {
            e.printStackTrace();                            // expect: print-stack-trace
        }
        Exception ex = new Exception("x");
        ex.printStackTrace();                               // expect: print-stack-trace
        Throwable t = ex.getCause();
        t.printStackTrace();                                // expect: print-stack-trace
        new RuntimeException("boom").printStackTrace();     // expect: print-stack-trace
        Runnable r = () -> {
            try { io(); } catch (Exception inner) { inner.printStackTrace(); }   // expect: print-stack-trace
        };
    }

    void negatives(Custom custom, java.io.PrintWriter pw) {
        try { io(); } catch (IOException e) {
            e.printStackTrace(pw);                          // explicit target stream
            log(e);
        }
        custom.printStackTrace();                           // unknown type
        printStackTrace();                                  // unqualified
    }

    void printStackTrace() {}
}
