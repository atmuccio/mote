class Durations {
    static final long YEAR_MILLIS = 365 * 24 * 60 * 60 * 1000;           // expect: int-constant-overflow
    static final long THREE_GB = 3 * 1024 * 1024 * 1024;                 // expect: int-constant-overflow
    static final int BIG = 2_000_000_000 + 2_000_000_000;                // expect: int-constant-overflow

    long wrong(long base) {
        long a = base + 1000 * 60 * 60 * 24 * 30;                         // expect: int-constant-overflow
        return a + (60 * 60 * 1000 * 1000);                               // expect: int-constant-overflow
    }

    long fine(int n) {
        long a = 365L * 24 * 60 * 60 * 1000;
        long b = 24 * 60 * 60 * 1000;
        int c = 0x7fffffff * 2;
        int d = Integer.MAX_VALUE + 1;
        long e = (long) 1000 * 1000 * 1000 * 10;
        int f = -2147483648;
        int g = 1 << 31;
        long h = n * 1000 * 1000;
        String s = "size " + 1024 * 1024;
        return a + b + c + d + e + f + g + h;
    }
}
