class Bits {
    long wrong(int value, long wide, short small, Integer boxed) {
        long mask = 1 << 40;                        // expect: shift-out-of-range
        int high = value >>> 32;                    // expect: shift-out-of-range
        long w = wide << 64;                        // expect: shift-out-of-range
        int s = small << 32;                        // expect: shift-out-of-range
        int neg = value >> -1;                      // expect: shift-out-of-range
        value <<= 33;                               // expect: shift-out-of-range
        int b = boxed << 35;                        // expect: shift-out-of-range
        return mask + high + w + s + neg + b;
    }

    long fine(int value, long wide, Unknown u, int n) {
        long a = 1L << 40;
        long b = wide >>> 32;
        int c = value << 31;
        int d = value >> n;
        long e = u.bits() << 40;
        long f = (long) value << 32;
        int g = value << 0;
        return a + b + c + d + e + f + g;
    }
}
