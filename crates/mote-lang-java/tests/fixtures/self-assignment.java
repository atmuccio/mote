import java.util.List;

class Point {
    private int x;
    private int y;
    private int[] data = new int[4];
    private Point next;

    Point(int x, int y) {
        x = x;                                  // expect: self-assignment
        this.y = y;
    }

    void positives(int i) {
        this.x = this.x;                        // expect: self-assignment
        data[i] = data[i];                      // expect: self-assignment
        next.x = next.x;                        // expect: self-assignment
        this . y = this.y;                      // expect: self-assignment
        y = (y);                                // expect: self-assignment
        next.next.y = next.next /* same */ .y;  // expect: self-assignment
    }

    void negatives(int x, int i, Point other, List<Point> list) {
        this.x = x;                             // parameter into field
        x = this.x;                             // field into parameter
        this.x += this.x;                       // compound assignment doubles the value
        data[i++] = data[i++];                  // side effects in the index
        data[i] = data[i + 1];
        other.x = next.x;
        list.get(0).x = list.get(0).x;          // calls may return different objects
        Runnable r = () -> { int z = 1; z = z + 1; };
        String s = "a";
        s = s.trim();
        Object o = new Object() {
            int w;
            void set(int w) { this.w = w; }
        };
    }

    record Pair(int a, int b) {
        Pair {
            a = Math.abs(a);
        }
    }
}

class Settings {
    private int limit;
    private int retries;

    void setLimit(int limt) {
        this.limit = limit;                     // expect: self-assignment
    }

    void setRetries(int retry) {
        retries = this.retries;                 // expect: self-assignment
    }

    void setBoth(int limit, int retryCount) {
        this.limit = limit;                     // the parameter
        int retries = retryCount;
        this.retries = retries;                 // the local
        java.util.function.IntConsumer c = retries2 -> this.retries = retries2;
        java.util.function.IntConsumer d = limit2 -> { int x = limit2; this.limit = x; };
        Object o = new Object() {
            int count;
            int limit;
            void set(int v) {
                this.count = count;             // expect: self-assignment
                this.limit = limit;             // expect: self-assignment
            }
        };
    }

    class Inner {
        void setLimit(int limt) {
            this.limit = limit;                 // `limit` is the outer class's field, not Inner's
        }
    }

    static class Child extends UnknownBase {
        void setSize(int sz) {
            this.size = size;                   // `size` may be inherited: unknown
        }
    }
}
