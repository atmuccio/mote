class Numbers {
    static final int SIZE = 1 * 60 * 1000;                         // literals only: fine
    static final long MB = 1 * 1024 * 1024L;

    int calc(int x, long y, double d, float f) {
        int a = x + 0;                                             // expect: arithmetic-no-op
        int a2 = 0 + x;                                            // expect: arithmetic-no-op
        int a3 = x - 0;                                            // expect: arithmetic-no-op
        long a4 = y * 1;                                           // expect: arithmetic-no-op
        long a5 = y / 1;                                           // expect: arithmetic-no-op
        int a6 = x | 0;                                            // expect: arithmetic-no-op
        int a7 = x ^ 0;                                            // expect: arithmetic-no-op
        int a8 = x & -1;                                           // expect: arithmetic-no-op
        long low = y & 0xFFFFFFFF;                                 // expect: arithmetic-no-op
        double a10 = d * 1;                                        // expect: arithmetic-no-op
        float a11 = f / 1;                                         // expect: arithmetic-no-op
        x += 0;                                                    // expect: arithmetic-no-op
        y *= 1;                                                    // expect: arithmetic-no-op
        return a;
    }

    int more(int x, int z) {
        int a9 = x & 0xFFFFFFFF;                                   // expect: arithmetic-no-op
        int a12 = (z + 1) * 1;                                     // expect: arithmetic-no-op
        return a9;
    }

    int fine(int x, long y, double d, char c, byte b, short s, String str, Integer boxed) {
        // Fine: negation, reciprocal, type changes, narrow types, strings, -0.0 normalization.
        int n1 = 0 - x;
        double n2 = 1 / d;
        long n3 = x + 0L;
        double n4 = x * 1.0;
        int n5 = c + 0;
        int n6 = b * 1;
        int n7 = s | 0;
        String n8 = str + 0;
        double n9 = d + 0;
        double n10 = d + 0.0;
        int n11 = boxed + 0;
        long n12 = y & 0xFFFFFFFFL;
        int n13 = x << 0;
        int n14 = x * 2;
        int n15 = x + 00;
        return n1;
    }

    void aligned(byte[] buf, int off, int v) {
        buf[off + 0] = (byte) v;
        buf[off + 1] = (byte) (v >> 8);
        int[] idx = { off + 0, off + 1, off + 2 };
        put(off + 0, off + 1);
    }

    void aligned2(int[] m, int row) {
        m[row * 4 + 0] = 1;
        m[row * 4 + 1] = 2;
    }

    void put(int a, int b) { }

    void gap(String[] cells, int i) {
        String a = cells[i + 0];
        if (a.isEmpty()) {
            return;
        }
        String b = cells[i + 1];
    }

    void loops(int[] rows) {
        for (int i = 0; i < 10; i++) {
            put(i + 29, 0);
        }
        for (int i = 0; i < 10; i++) {
            put(i + 0, 0);
        }
    }

    void twice(char[] content, int end) {
        draw(content, 0, end - 0);                                 // expect: arithmetic-no-op
        draw(content, 0, end - 0);                                 // expect: arithmetic-no-op
    }

    void draw(char[] c, int a, int b) { }

    long shifted(long next) {
        return next++ >> 32 & 0xFFFFFFFF;                          // expect: arithmetic-no-op
    }

    void rounding(int total, int pageSize, long big, double d) {
        int pages = (int) Math.ceil(total / pageSize);             // expect: arithmetic-no-op
        long r = Math.round(big);                                  // expect: arithmetic-no-op
        double fl = Math.floor(total);                             // expect: arithmetic-no-op
        double ok = Math.ceil((double) total / pageSize);
        long ok2 = Math.round(d);
        double ok3 = Math.floor(d / 2);
        double ok4 = Math.ceil(Math.abs(Unknown.approx(d)));
    }

    void unknown(Object o, Foo foo) {
        int u = foo.value() + 0;
    }
}
