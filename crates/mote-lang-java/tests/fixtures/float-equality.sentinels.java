class Sentinels {
    public static final float AUTO_DPI = -1f;
    static final double NONE = 0;
    static final double RATIO = 0.5;
    private final double unset = -1;

    float dpi(float requested) {
        if (requested == AUTO_DPI) {
            return 72f;
        }
        return requested;
    }

    boolean none(double v) {
        return v != NONE;
    }

    boolean ratio(double v) {
        return v == RATIO;                          // expect: float-equality
    }

    boolean instanceSentinel(double v) {
        return v == unset;                          // expect: float-equality
    }

    float interpolate(float x, float xMin, float xMax, float yMin, float yMax) {
        if (xMax == xMin) {
            return yMin;
        }
        return yMin + (x - xMin) * (yMax - yMin) / (xMax - xMin);
    }

    double reversed(double lo, double hi, double v) {
        if (lo == hi) {
            return 0;
        }
        return (v - lo) / (hi - lo);
    }

    double noDivision(double lo, double hi) {
        if (lo == hi) {                             // expect: float-equality
            return 0;
        }
        return hi - lo;
    }
}

interface Limits {
    double UNLIMITED = -1;
}

class UsesInterface implements Limits {
    boolean unlimited(double v) {
        return v == UNLIMITED;
    }
}
