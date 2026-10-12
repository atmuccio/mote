import java.util.function.*;

class Params {
    int field;

    String normalize(String name, int count, int... rest) {
        name = name.trim();                                  // expect: parameter-reassignment
        count++;                                             // expect: parameter-reassignment
        --count;                                             // expect: parameter-reassignment
        count += 2;                                          // expect: parameter-reassignment
        rest = new int[0];                                   // expect: parameter-reassignment
        this.field = count;
        field = count;
        rest[0] = 1;                                         // element write, not a reassignment
        return name;
    }

    Params(int field) {
        this.field = field;
        field = 0;                                           // expect: parameter-reassignment
    }

    void retry(int retryLimit) {
        retryLimit = retryLimit;                             // a self-assignment: reported by that rule only
        retryLimit = (retryLimit);
    }

    void locals(int p) {
        int local = p;
        local = 5;
        for (int i = 0; i < p; i++) {}
        try {
            p();
        } catch (RuntimeException e) {
            e = null;
        }
        for (String s : new String[0]) {
            s = s.trim();
        }
    }

    void lambdas() {
        Function<Integer, Integer> f = x -> {
            x = x + 1;
            return x;
        };
        BiFunction<Integer, Integer, Integer> g = (Integer a, Integer b) -> {
            a = b;
            return a;
        };
    }

    void anonymous(int value) {
        Runnable r = new Runnable() {
            int value;
            public void run() {
                value = 1;                                   // the anonymous class's field
            }
        };
    }

    void p() {}

    record Range(int lo, int hi) {
        Range {
            lo = Math.min(lo, hi);                           // compact constructor normalization
        }
        Range(int lo) {
            this(lo, lo);
        }
        int with(int lo) {
            lo = lo + 1;                                     // expect: parameter-reassignment
            return lo;
        }
    }
}
