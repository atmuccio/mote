import java.util.*;
import java.util.function.*;

class Ternaries {
    String size(int n) {
        String a = n > 100 ? "large" : n > 10 ? "medium" : "small";     // expect: nested-ternary
        String b = n > 0 ? (n > 5 ? "big" : "small") : "none";          // expect: nested-ternary
        String c = (n > 0 ? true : false) ? "x" : "y";                  // expect: nested-ternary
        String d = n > 0 ? String.valueOf(n > 9 ? 9 : n) : "";          // expect: nested-ternary
        String e = n > 1 ? "a" : n > 2 ? "b" : n > 3 ? "c" : "d";        // expect: nested-ternary, nested-ternary
        String f = n > 0 ? "pos" : "neg";
        String g = f.isEmpty() ? "empty" : f;
        Function<Integer, String> h = n > 0 ? x -> x > 0 ? "p" : "n" : x -> "z";
        Object i = n > 0 ? new Object() {
            String s() { return n > 1 ? "a" : "b"; }
        } : null;
        int j = n > 0
            ? 1
            : n < -10                                                    // expect: nested-ternary
                ? -2
                : -1;
        return a + b + c + d + e + g + h + i + j;
    }
}
