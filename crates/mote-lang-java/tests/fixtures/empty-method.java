import java.awt.event.*;

abstract class Empties implements KeyListener {
    // expect-next-line: empty-method
    public void doNothing() {}
    // expect-next-line: empty-method
    void alsoEmpty() {
    }
    @Override
    // expect-next-line: empty-method
    public void keyPressed(KeyEvent e) {}
    @Override
    public void keyReleased(KeyEvent e) {
        // not interested in releases
    }
    @Override
    public void keyTyped(KeyEvent e) {
        /* intentionally empty */
    }
    void reset() {} // no state to reset
    abstract void hook();
    native void nativeCall();
    Empties() {}
    void notEmpty() { System.out.println(); }

    @org.aspectj.lang.annotation.Pointcut("execution(* *(..))")
    public void anyMethod() {}
    @org.junit.Before
    public void setUp() {}

    interface Api {
        default void onEvent() {}
        void abstractOne();
        // expect-next-line: empty-method
        static void staticEmpty() {}
    }

    Runnable r = new Runnable() {
        // expect-next-line: empty-method
        public void run() {}
    };

    enum Mode {
        A {
            // expect-next-line: empty-method
            void apply() {}
        };
        void apply() {
            // default: nothing
        }
    }

    record Rec(int a) {
        // expect-next-line: empty-method
        void check() {}
    }
}
