import java.io.Reader;

class OverloadsNotAdjacent {
    private int state;

    OverloadsNotAdjacent() {
    }

    void parse(String text) {
    }

    void reset() {
    }

    void parse(Reader in) {                                   // expect: overloads-not-adjacent
    }

    void parse(char[] chars) {
    }

    private int helper;

    OverloadsNotAdjacent(int state) {                         // expect: overloads-not-adjacent
        this.state = state;
    }

    void parse(byte[] bytes) {                                // expect: overloads-not-adjacent
    }

    void together(int a) {
    }

    void together(long a) {
    }

    void together(String a) {
    }

    static class Nested {
        void reset() {                                        // another type's methods are apart
        }
    }

    interface Api {
        void send(String s);

        void flush();

        void send(byte[] b);                                  // expect: overloads-not-adjacent
    }

    enum Mode {
        A, B;

        void apply(int x) {
        }

        int field;

        void apply(String x) {                                // expect: overloads-not-adjacent
        }
    }

    Object anonymous = new Object() {
        void run(int x) {
        }

        void stop() {
        }

        void run(String x) {                                  // expect: overloads-not-adjacent
        }
    };
}
