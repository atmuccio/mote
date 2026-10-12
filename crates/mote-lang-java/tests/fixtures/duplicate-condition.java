import java.io.BufferedReader;
import java.io.IOException;

class Chains {
    static final int FILE = 1, DIRECTORY = 2, LINK = 3;

    void positives(int type, String s, Chains c) {
        if (type == FILE) {
            open();
        } else if (type == DIRECTORY) {
            list();
        } else if (type == FILE) {                  // expect: duplicate-condition
            link();
        }

        if (s.isEmpty()) {
            open();
        } else if (s . isEmpty()) {                 // expect: duplicate-condition
            list();
        } else {
            link();
        }

        if (c.ready() && type > 0) open();
        else if ((c.ready() && type > 0)) list();   // expect: duplicate-condition
        else if (type < 0) link();
        else if (type < 0) link();                  // expect: duplicate-condition
    }

    void negatives(int type, String s, BufferedReader r) throws IOException {
        if (type == FILE) {
            open();
        } else if (type == LINK) {
            link();
        }
        if (s.equals("a b")) {
            open();
        } else if (s.equals("ab")) {
            list();
        }
        // Separate statements, not one chain.
        if (type == FILE) open();
        if (type == FILE) list();
        // Nested if inside an else block is a new chain.
        if (type == FILE) {
            open();
        } else {
            if (type == FILE) {
                list();
            }
        }
        // Conditions with side effects are evaluated again and may differ.
        String line;
        if ((line = r.readLine()) == null) {
            open();
        } else if ((line = r.readLine()) == null) {
            list();
        }
        if (type-- > 0) {
            open();
        } else if (type-- > 0) {
            list();
        }
    }

    boolean ready() { return true; }
    void open() {}
    void list() {}
    void link() {}
}
