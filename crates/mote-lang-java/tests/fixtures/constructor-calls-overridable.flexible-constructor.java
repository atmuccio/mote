// Java 25 flexible constructor bodies: the statements before `super(...)` / `this(...)` cannot
// call instance methods, so only the calls after it can reach an override.
class Gauge extends Dial {
    private final int max;

    Gauge(int max) {
        if (max <= 0) {
            throw new IllegalArgumentException("max");
        }
        this.max = max;
        super(clamp(max));
        redraw();                                                    // expect: constructor-calls-overridable
    }

    Gauge(String max) {
        int parsed = Integer.parseInt(max.strip());
        this(parsed);
        redraw();                                                    // expect: constructor-calls-overridable
    }

    Gauge() {
        this(clamp(10));
    }

    protected void redraw() {}

    static int clamp(int n) {
        return Math.min(n, 100);
    }
}

class Dial {
    Dial(int size) {}
}
