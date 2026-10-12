// mote-test: params java_release=16
// Java 16 is the first release with pattern matching for `instanceof`.
class Boundary {
    int length(Object value) {
        if (value instanceof String) {                                      // expect: instanceof-pattern-candidate
            return ((String) value).length();
        }
        return 0;
    }
}
