// mote-test: path src/main/java/demo/Single.java
package demo;

import java.util.List;

/** One type with nested types: fine. */
public final class Single {
    static class Nested {
    }

    interface Callback {
    }

    enum Mode { A, B }

    void method() {
        class Local {
        }
        Runnable r = new Runnable() {
            @Override
            public void run() {
            }
        };
    }
}
