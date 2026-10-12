// mote-test: params max=3
class Small {
    int a, b;
    String c;
    static final int K1 = 1, K2 = 2;
    static final String K3 = "x";
}

class Big {                                                      // expect: too-many-fields
    int a, b;
    String c;
    static int counter;
    class Inner {                                                // expect: too-many-fields
        int x, y, z, w;
    }
}

class InnerHolder {
    static class Deep {                                          // expect: too-many-fields
        int a, b, c, d;
    }
}

enum BigEnum {                                                   // expect: too-many-fields
    A;
    int a;
    int b;
    final int c = 0;
    long d;
}

interface Constants {
    int A = 1, B = 2, C = 3, D = 4;
}
