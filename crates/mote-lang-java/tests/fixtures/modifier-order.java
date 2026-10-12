import java.util.List;

abstract class ModifierOrder {
    final static public int LIMIT = 10;                       // expect: modifier-order
    public static final int GOOD = 1;
    static public String name;                                // expect: modifier-order
    private static final long serialVersionUID = 1L;
    final private List<String> items = List.of();             // expect: modifier-order
    transient volatile int counter;
    volatile transient int other;                             // expect: modifier-order

    synchronized public void reset() {                        // expect: modifier-order
    }

    public synchronized void fine() {
    }

    abstract protected void hook();                           // expect: modifier-order

    @Override
    public final String toString() {
        return "x";
    }

    @Deprecated final public void old() {                     // expect: modifier-order
    }

    public @Deprecated static void annotationBetween() {      // annotations anywhere are fine
    }

    static abstract class Inner {                             // expect: modifier-order
    }

    abstract static class InnerFine {
    }

    interface Api {
        default public void run() {                           // expect: modifier-order
        }

        public default void ok() {
        }

        static void util() {
        }
    }

    void locals() {
        final int x = 1;
        @SuppressWarnings("unused") final int y = 2;
    }
}

sealed abstract class Shape permits Circle {                  // expect: modifier-order
}

final class Circle extends Shape {
}

non-sealed abstract class Open extends Shape {                // expect: modifier-order
}
