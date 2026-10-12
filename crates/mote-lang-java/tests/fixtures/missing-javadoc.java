// mote-test: path src/main/java/demo/RateLimiter.java
package demo;

import java.util.List;

public final class RateLimiter {                              // expect: missing-javadoc
    public static final int DEFAULT = 10;                     // expect: missing-javadoc
    /** Documented field. */
    public static final int MAX = 100;
    protected int permits;                                    // expect: missing-javadoc
    private int used;
    int packagePrivate;
    private String name;
    private boolean open;

    public RateLimiter() {                                    // expect: missing-javadoc
    }

    /** Documented constructor. */
    public RateLimiter(int permits) {
        this.permits = permits;
    }

    public boolean tryAcquire(int n) {                        // expect: missing-javadoc
        return n < permits;
    }

    protected void refill() {                                 // expect: missing-javadoc
    }

    /**
     * Documented method.
     *
     * @return the count
     */
    public int used() {
        return used;
    }

    @Deprecated
    /** A doc comment after the annotation counts here (another rule reports its position). */
    public void legacy() {
    }

    /** Documented, then annotated. */
    @Deprecated
    public void old() {
    }

    // A line comment between the doc comment and the method does not hide the doc comment.
    /** Documented. */
    // note
    public void noted() {
    }

    /* A plain block comment is not documentation. */
    public void plain() {                                     // expect: missing-javadoc
    }

    @Override
    public String toString() {
        return name;
    }

    public String getName() {
        return name;
    }

    public boolean isOpen() {
        return this.open;
    }

    public void setName(String name) {
        this.name = name;
    }

    public RateLimiter setUsed(int used) {
        this.used = used;
        return this;
    }

    public String getLabel() {                                // expect: missing-javadoc
        return name.trim();
    }

    public void setOpen(boolean open) {                       // expect: missing-javadoc
        this.open = open;
        used = 0;
    }

    private void hidden() {
    }

    void packageMethod() {
    }

    public static class Builder {                             // expect: missing-javadoc
        public Builder limit(int n) {                         // expect: missing-javadoc
            return this;
        }
    }

    static class Hidden {
        public void notVisible() {
        }
    }

    private static class Secret {
        public void notVisible() {
        }
    }

    /** Documented interface. */
    public interface Listener {
        void onEvent(String e);                               // expect: missing-javadoc

        int CONSTANT = 1;                                     // expect: missing-javadoc

        private void helper() {
        }

        /** Documented. */
        default void ok() {
        }

        @Deprecated
        /** After the annotation, no keyword modifier: counts here. */
        void legacy();
    }

    /** Documented enum. */
    public enum Mode {
        FAST, SLOW;

        public int weight() {                                 // expect: missing-javadoc
            return 1;
        }
    }

    /** Documented record. */
    public record Window(int size) {
        public Window {
        }

        public int doubled() {                                // expect: missing-javadoc
            return size * 2;
        }
    }

    public void anonymous() {                                 // expect: missing-javadoc
        Runnable r = new Runnable() {
            public void run() {
            }
        };
        class Local {
            public void notVisible() {
            }
        }
    }
}

class PackagePrivateType {
    public void notVisible() {
    }
}
