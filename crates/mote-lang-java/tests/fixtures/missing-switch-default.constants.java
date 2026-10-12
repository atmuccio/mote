import static com.acme.Codes.OK;
import static com.acme.Codes.FAIL;

import com.acme.Status;

class Bands {
    static final int FIRST = 1;
    static final int SECOND = 2;
    static final String READ = "r";
    static final String WRITE = "w";
    private String mode;

    static class Names {
        static final String OPEN = "OPEN";
        static final String CLOSED = "CLOSED";
    }

    void band(final int kind) {
        switch (kind) {                                             // expect: missing-switch-default
            case FIRST:
                first();
                break;
            case SECOND:
                second();
                break;
        }
    }

    void access() {
        switch (this.mode) {                                        // expect: missing-switch-default
            case READ -> first();
            case WRITE -> second();
        }
    }

    void traps(int code, Status status, Object o) {
        // Labels come from another class: they might be the constants of an enum.
        switch (code) {
            case OK:
                first();
                break;
            case FAIL:
                second();
                break;
        }
        // The selector is a method result: an enum of another file may have constants with
        // the same names as the String constants of this file.
        switch (status()) {
            case OPEN:
                first();
                break;
            case CLOSED:
                second();
                break;
        }
        // Declared with a type of another file.
        switch (status) {
            case OPEN:
                first();
                break;
        }
    }

    Status status() {
        return null;
    }

    void first() { }
    void second() { }
}
