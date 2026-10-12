import java.util.List;

class Plotter {
    void plot(List<Double> values, double start, double end, double step) {
        for (double x = 0.0; x <= 1.0; x += 0.1) {                  // expect: float-loop-counter
            draw(x);
        }
        for (float f = 0; f < 10; f++) {                            // expect: float-loop-counter
            draw(f);
        }
        for (double t = start; t < end; t = t + step) {             // expect: float-loop-counter
            draw(t);
        }
        for (var v = 0.5; v > 0.001; v /= 2) {                      // expect: float-loop-counter
            draw(v);
        }
        for (int i = 0, j = 0; i < 3; i++) {
            draw(j);
        }
        for (Double boxed = 1.0; boxed < 2.0; boxed += 0.25) {      // expect: float-loop-counter
            draw(boxed);
        }

        // Not reported.
        for (int i = 0; i <= 10; i++) {
            draw(i / 10.0);
        }
        for (double d : values) {
            draw(d);
        }
        for (double scale = 2.0; running(); tick()) {               // not updated by the loop header
            draw(scale);
        }
        for (var k = 0; k < 3; k++) {
            draw(k);
        }
        for (long n = 0; n < 5; n++) {
            draw(n);
        }
        for (double total = 0, part;; total += part) {                // an accumulator, not tested
            part = next();
            if (part < 0) {
                break;
            }
        }
        for (double w = 0; running(); w += 0.5) {                   // not tested by the condition
            draw(w);
        }
        double y = 0;
        for (; y < 1; y += 0.1) {                                   // declared outside the header
            draw(y);
        }
    }

    boolean running() {
        return false;
    }

    double next() {
        return 1;
    }

    void tick() {
    }

    void draw(double v) {
    }
}
