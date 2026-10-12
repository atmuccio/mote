class record {                                                      // expect: restricted-identifier
}

class var {                                                         // expect: restricted-identifier
}

interface sealed {                                                  // expect: restricted-identifier
}

enum yield { A }                                                    // expect: restricted-identifier

@interface permits {                                                // expect: restricted-identifier
}

class Box<var> {                                                    // expect: restricted-identifier
    <record> void put(record r) {                                   // expect: restricted-identifier
    }
}

class Worker extends Thread {
    public void run() {
        yield();                                                    // expect: restricted-identifier
        Runnable r = () -> yield(4);                                // expect: restricted-identifier
        yield(5);                                                   // expect: restricted-identifier
    }

    static void yield(int n) {
    }
}

// Compliant
class Legal {
    int var = 1;                                                    // variables stay legal
    String record = "r";
    boolean sealed;

    void permits() {
    }

    int values(int k) {
        Thread.yield();
        this.yield2();
        int x = switch (k) {
            case 1 -> {
                yield (k + 1);                                      // a real yield statement
            }
            default -> 0;
        };
        java.util.function.IntUnaryOperator f = _ -> 1;             // unnamed (Java 22) or old: release unknown
        return x;
    }

    void yield2() {
    }

    record Point(int x, int y) {
    }
}
