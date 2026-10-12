import java.io.BufferedReader;
import java.io.IOException;

class Conditions {
    private boolean ready;
    private int flags;

    void positives(boolean found, boolean done) {
        if (found = true) {                         // expect: assignment-in-condition
            start();
        }
        while (done = check()) {                    // expect: assignment-in-condition
            start();
        }
        do {
            start();
        } while (ready = check());                  // expect: assignment-in-condition
        if (this.ready = found) start();            // expect: assignment-in-condition
        if (ready |= found) start();                // expect: assignment-in-condition
    }

    void negatives(BufferedReader r, boolean ok) throws IOException {
        String line;
        while ((line = r.readLine()) != null) {
            start();
        }
        if ((ok = check())) {                       // extra parentheses: intentional
            start();
        }
        if (ok == check()) {
            start();
        }
        int n;
        if ((n = flags) > 0) {
            start();
        }
        boolean b = (ok = check()) ? true : false;
        for (int i = 0; (i = next(i)) < 10;) {
            start();
        }
        Runnable run = () -> { boolean x; if (x = check()) start(); };  // expect: assignment-in-condition
    }

    boolean check() { return true; }
    int next(int i) { return i + 1; }
    void start() {}
}
