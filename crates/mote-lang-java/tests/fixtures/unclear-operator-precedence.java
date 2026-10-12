class UnclearOperatorPrecedence {
    boolean logic(boolean admin, boolean active, boolean override, boolean guest) {
        boolean a = admin && active || override;                     // expect: unclear-operator-precedence
        boolean b = override || admin && active;                     // expect: unclear-operator-precedence
        boolean c = admin && active || guest && override;            // expect: unclear-operator-precedence, unclear-operator-precedence
        boolean d = admin || active || override && guest;            // expect: unclear-operator-precedence
        return a;
    }

    int bits(int x, int n, int bit, int hi, int lo, boolean f, int y) {
        int a = 1 << bit + 1;                                        // expect: unclear-operator-precedence
        int b = x + n >> 1;                                          // expect: unclear-operator-precedence
        int c = x & 0xFF | hi;                                       // expect: unclear-operator-precedence
        int d = x ^ y & 0x0F;                                        // expect: unclear-operator-precedence
        boolean e = f & x == y;                                      // expect: unclear-operator-precedence
        boolean g = f | x < y;                                       // expect: unclear-operator-precedence
        return a;
    }

    // Compliant
    int fine(boolean p, boolean q, boolean r, int x, int y, int hi, int lo) {
        boolean a = (p && q) || r;
        boolean b = p || q || r;
        boolean c = p && q && r;
        boolean d = x > 0 && y < 0 || r == p;                        // expect: unclear-operator-precedence
        boolean e = x > 0 && y < 0;
        int f = 1 << (x + 1);
        int g = (x + y) >> 1;
        int h = x + y * 2;
        int i = hi << 8 | lo;                                        // shift inside a bitwise operator
        int j = (x & 0xFF) | hi;
        int k = x & y & 0x0F;
        int l = x | y | hi;
        boolean m = (p & (x == y));
        String s = "a" + x + y;
        return f;
    }
}
