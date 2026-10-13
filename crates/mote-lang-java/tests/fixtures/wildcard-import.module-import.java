import module java.base;
import java.util.*;                                                 // expect: wildcard-import
import static java.util.Objects.*;                                  // expect: wildcard-import

// A module import (Java 25) is a different declaration and is not reported.
class Modules {
    List<String> names;
}
