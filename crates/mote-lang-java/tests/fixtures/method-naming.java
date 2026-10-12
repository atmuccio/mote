import java.util.function.Supplier;

class Methods implements Runnable {
    Methods() {}                                   // constructors are skipped
    void doWork() {}
    void Do_Work() {}                              // expect: method-naming
    int GetCount() { return 0; }                   // expect: method-naming
    void get_count() {}                            // expect: method-naming
    void parseHTTP2() {}
    boolean isUpgradeFromPre3_8() { return true; }   // underscore between digits: version number
    boolean isPre3_() { return true; }             // expect: method-naming
    void größeÄndern() {}                          // Unicode lowercase first letter
    void x() {}
    static void $helper() {}                       // expect: method-naming

    @Override
    public void run() {}

    @Override
    public String toString() { return ""; }

    native void Native_Call();                     // JNI: the name is dictated externally

    interface Api {
        void Fetch_All();                          // expect: method-naming
        default void fine() {}
    }

    Supplier<Object> s = new Supplier<>() {
        @Override
        public Object get() { return null; }
        void Bad_Name() {}                         // expect: method-naming
    };

    enum E {
        A;
        void Weird() {}                            // expect: method-naming
    }

    record R(int a) {
        int A() { return a; }                      // expect: method-naming
    }
}

@interface Ann {
    String value();                                // annotation elements are not methods here
}
