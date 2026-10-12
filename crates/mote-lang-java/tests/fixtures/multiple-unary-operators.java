import java.util.Set;

class Flags {
    boolean check(Set<String> flags, boolean ready, int offset, int mask) {
        boolean active = !!flags.contains("active");                // expect: multiple-unary-operators
        int n = - -offset;                                          // expect: multiple-unary-operators
        int m = ~~mask;                                             // expect: multiple-unary-operators
        int next = -~offset;                                        // expect: multiple-unary-operators
        boolean triple = !!!ready;                                  // expect: multiple-unary-operators
        int plus = + +offset;                                       // expect: multiple-unary-operators

        // Not reported.
        int dec = --offset;                                         // a decrement
        int negDec = -offset--;
        int paren = -(-offset);                                     // explicit nesting
        boolean notParen = !(!ready);
        int neg = -offset;
        boolean not = !ready;
        int minusLiteral = -1;
        int sub = offset - -1;                                      // binary minus, then a negative literal
        Runnable r = () -> System.out.println(!ready);
        return active && triple && n + m + next + plus + dec + negDec + paren + neg + minusLiteral + sub > 0 && notParen && not;
    }
}
