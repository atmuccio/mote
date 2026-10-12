import java.util.regex.Pattern;

class Counters {
    static int created;
    int own;
    static final String PREFIX = "c-";

    static Counters make() {
        return new Counters();
    }

    void bump() {
        created++;
    }

    static void reset(int value) {
        created = value;
    }

    void reset() {
        own = 0;
    }
}

class SpecialCounters extends Counters {
    static String describe() {
        return "special";
    }
}

class Worker extends Thread {
    @Override
    public void run() {
    }
}

class Uses {
    void positives(Counters counters, Thread worker, Worker mine, Pattern pattern, String text)
            throws InterruptedException {
        int n = counters.created;                                   // expect: static-access-through-instance
        Counters c = counters.make();                               // expect: static-access-through-instance
        counters.reset(3);                                          // expect: static-access-through-instance
        worker.sleep(100);                                          // expect: static-access-through-instance
        if (worker.interrupted()) {                                 // expect: static-access-through-instance
            return;
        }
        mine.sleep(1);                                              // expect: static-access-through-instance
        Pattern p = pattern.compile("x");                           // expect: static-access-through-instance
        String v = text.valueOf(3);                                 // expect: static-access-through-instance
        int m = SpecialCounters.created;                            // expect: static-access-through-instance
        Counters d = SpecialCounters.make();                        // expect: static-access-through-instance
        Thread t = Worker.currentThread();                          // expect: static-access-through-instance
        String s = new Counters().PREFIX;                           // expect: static-access-through-instance
    }

    void negatives(Counters counters, Thread worker, Integer boxed, Double d, Unknown u) {
        int n = Counters.created;
        Counters.reset(1);
        counters.reset();                                           // instance overload
        counters.bump();
        int own = counters.own;
        String describe = SpecialCounters.describe();
        Thread.sleep(1);
        boolean alive = worker.isAlive();
        String s = boxed.toString();                                 // instance toString()
        boolean nan = d.isNaN();                                     // instance isNaN()
        u.sleep(5);                                                  // unknown type
        String t = String.valueOf(5);
    }

    void self() {
        this.helper();
    }

    static void helper() {
    }
}
