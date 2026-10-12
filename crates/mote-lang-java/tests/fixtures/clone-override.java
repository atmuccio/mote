class Order implements Cloneable {
    private java.util.List<String> lines;

    @Override
    public Order clone() {                                          // expect: clone-override
        try {
            return (Order) super.clone();
        } catch (CloneNotSupportedException e) {
            throw new AssertionError(e);
        }
    }

    record Point(int x, int y) {
        public Point clone() { return new Point(x, y); }            // expect: clone-override
    }

    Object anon = new Object() {
        @Override protected Object clone() { return this; }         // expect: clone-override
    };

    // negatives
    static final class Singleton {
        @Override
        protected Object clone() throws CloneNotSupportedException {
            throw new CloneNotSupportedException();                 // cloning forbidden
        }
    }

    interface Copyable<T> {
        T clone();                                                  // no body
    }

    abstract static class Shape {
        public abstract Shape clone();
    }

    Order clone(boolean deep) { return this; }                      // overload

    Order copy() {
        Order o = new Order();
        o.lines = new java.util.ArrayList<>(lines);
        return o;
    }

    Object callsClone(int[] a) { return a.clone(); }
}
