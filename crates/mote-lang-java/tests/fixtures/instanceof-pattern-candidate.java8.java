// mote-test: params java_release=8
// Java 8 (declared as `1.8` in the build file, normalized to 8): nothing is reported.
import java.util.List;

class Visitor {
    int count(Object node) {
        if (node instanceof List) {
            return ((List) node).size();
        }
        if ((node instanceof String)) return ((String) node).length();
        return 0;
    }
}
