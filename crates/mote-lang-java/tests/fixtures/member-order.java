class MemberOrder {
    private static final int MAX = 64;
    private final String id;
    private int count;

    MemberOrder(String id) {
        this.id = id;
    }

    String id() {
        return id;
    }

    private static final int MIN = 1;                         // expect: member-order
    private int late;                                         // expect: member-order

    MemberOrder() {                                           // expect: member-order
        this("x");
    }

    static class Nested {
        int a;
        static int B;                                         // expect: member-order

        void m() {
        }
    }

    interface Api {
        void run();

        int CONSTANT = 1;                                     // expect: member-order
    }

    enum Mode {
        A, B;

        private final int weight = 1;

        Mode() {
        }

        static final int LIMIT = 3;                           // expect: member-order

        int weight() {
            return weight;
        }
    }

    record Point(int x, int y) {
        static final Point ORIGIN = new Point(0, 0);

        Point {
        }

        int sum() {
            return x + y;
        }
    }

    static {
        System.out.println();
    }

    Object anonymous = new Object() {                         // expect: member-order
        void m() {
        }

        int notChecked;
    };
}
