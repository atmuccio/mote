class Longs {
    long a = 10l;                  // expect: long-literal-lowercase
    long b = 0xFFl;                // expect: long-literal-lowercase
    long c = 0b101l;               // expect: long-literal-lowercase
    long d = 1_000l;               // expect: long-literal-lowercase
    long e = -5l;                  // expect: long-literal-lowercase
    long ok1 = 10L;
    long ok2 = 0xFFL;
    int ok3 = 10;
    float f = 1.5f;
    double g = 2.0d;
    String s = "10l";
    char ch = 'l';
    long ok4 = 017L;
    long h = 017l;                 // expect: long-literal-lowercase
}
