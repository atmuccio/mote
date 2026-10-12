// mote-test: params java_release=17
import java.util.List;

class Shapes {
    double area(Object shape) {
        if (shape instanceof Circle) {                                      // expect: instanceof-pattern-candidate
            Circle c = (Circle) shape;
            return c.radius() * c.radius() * Math.PI;
        }
        if (shape instanceof Square && ((Square) shape).side() > 0) {       // expect: instanceof-pattern-candidate
            return ((Square) shape).side();
        }
        if ((shape instanceof Rect)) return ((Rect) shape).w();             // expect: instanceof-pattern-candidate
        if (this.cached instanceof java.util.Map) {                         // expect: instanceof-pattern-candidate
            return ((java.util.Map<?, ?>) this.cached).size() + ((java.util.Map) this.cached).size();
        }
        return 0;
    }

    Object cached;

    double negatives(Object shape, Object other) {
        if (shape instanceof Circle c) {                                    // already a pattern
            return c.radius();
        }
        if (shape instanceof Circle) {                                      // no cast
            return 1;
        }
        if (shape instanceof Circle) {                                      // cast of another variable
            return ((Circle) other).radius();
        }
        if (shape instanceof List) {                                        // different type text
            return ((List<String>) shape).size();
        }
        if (shape instanceof Circle) {
            return 0;
        } else {
            return ((Circle) shape).radius();                               // else branch
        }
    }

    double moreNegatives(Object shape) {
        if (!(shape instanceof Circle)) {                                   // negated
            return ((Circle) shape).radius();
        }
        if (shape instanceof Circle || shape instanceof Square) {           // disjunction
            return ((Circle) shape).radius();
        }
        boolean b = shape instanceof Circle;
        if (getShape() instanceof Circle) {                                 // method call, not a variable
            return ((Circle) getShape()).radius();
        }
        if (shape instanceof Circle) {
            Runnable r = new Runnable() {
                public void run() { Circle c = (Circle) shape; }             // inner class body
            };
        }
        return 0;
    }
}
