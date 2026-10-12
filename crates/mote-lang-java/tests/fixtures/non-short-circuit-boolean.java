import java.util.List;

class ShortCircuit {
    private boolean ready;
    private Boolean boxed;

    boolean positives(String s, boolean a, boolean b, List<String> list) {
        boolean r1 = s != null & s.isEmpty();               // expect: non-short-circuit-boolean
        boolean r2 = a | b;                                 // expect: non-short-circuit-boolean
        if (list.isEmpty() | ready) {                       // expect: non-short-circuit-boolean
            return true;
        }
        boolean r3 = (a & b) | ready;                       // expect: non-short-circuit-boolean, non-short-circuit-boolean
        boolean r4 = boxed & a;                             // expect: non-short-circuit-boolean
        return r1 && r2 && r3 && r4;
    }

    int negatives(int x, int y, long mask, boolean a, Unknown u, List<Boolean> flags) {
        int bits = x & y;
        long m = mask | 0x10L;
        boolean ok = true;
        ok &= a;                                            // compound assignment
        ok |= check();
        boolean c = u.flag() & a;                           // unknown type
        boolean d = a && check();
        flags.forEach(f -> System.out.println(f | a));     // lambda parameter type is unknown
        return bits + (int) m + (ok ? 1 : 0) + (c ? 1 : 0) + (d ? 1 : 0);
    }

    boolean check() { return true; }
}
