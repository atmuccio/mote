class NegatedComparison {
    boolean wrong(int count, long size, Integer boxed, Object a, Object b, char c) {
        boolean x = !(count == 0);                                  // expect: negated-comparison
        boolean y = !(a != b);                                      // expect: negated-comparison
        boolean z = !(count >= size);                               // expect: negated-comparison
        boolean w = !((boxed < 3));                                 // expect: negated-comparison
        boolean v = !(c > 'a');                                     // expect: negated-comparison
        double d = 0.5;
        boolean u = !(d == 1.0);                                    // expect: negated-comparison
        return x && y && z && w && v && u;
    }

    // Compliant
    boolean fine(double d, float f, int i, Unknown u, boolean p, boolean q) {
        boolean a = !(d < 1.0);                                     // NaN: not the same as >=
        boolean b = !(f >= 0f);
        boolean c = !(i < u.value());                               // unknown operand type
        boolean e = !(p && q);
        boolean g = !p;
        boolean h = i != 0;
        boolean k = !(i < 3 && p);
        return a && b && c && e && g && h && k;
    }

    interface Unknown { Object value(); }
}
