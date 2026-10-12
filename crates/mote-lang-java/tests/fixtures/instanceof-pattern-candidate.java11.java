// mote-test: params java_release=11
// A project compiled for Java 11: `x instanceof T name` does not compile there, so the usual
// check-then-cast is the only way to write it. Nothing is reported.
import java.util.Map;

class LegacyShapes {
    Object cached;

    double area(Object shape) {
        if (shape instanceof Circle) {
            Circle c = (Circle) shape;
            return c.radius() * c.radius() * Math.PI;
        }
        if (shape instanceof Square && ((Square) shape).side() > 0) {
            return ((Square) shape).side();
        }
        if (this.cached instanceof Map) {
            return ((Map<?, ?>) this.cached).size();
        }
        return 0;
    }

    interface Circle {
        double radius();
    }

    interface Square {
        double side();
    }
}
