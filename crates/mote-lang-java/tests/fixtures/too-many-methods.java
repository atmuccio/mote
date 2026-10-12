// mote-test: params max=3
class Small {
    Small() {}
    Small(int a) {}
    void a() {}
    void b() {}
    void c() {}
}

class Big {                                                      // expect: too-many-methods
    void a() {}
    void b() {}
    void c() {}
    void d() {}
    class Nested {                                               // nested methods count for Nested only
        void e() {}
    }
    Runnable r = new Runnable() {
        public void run() {}
        void f() {}
        void g() {}
        void h() {}
    };
}

class Wrapper implements Runnable, Comparable<Wrapper> {
    @Override public void run() {}
    @Override public int compareTo(Wrapper o) { return 0; }
    @Override public String toString() { return ""; }
    @Override public int hashCode() { return 0; }
    void own() {}
}

interface ManyMethods {
    void a();
    void b();
    void c();
    void d();
}

enum BigEnum {                                                   // expect: too-many-methods
    A;
    void a() {}
    void b() {}
    void c() {}
    void d() {}
}

record BigRecord(int x) {                                        // expect: too-many-methods
    void a() {}
    void b() {}
    void c() {}
    void d() {}
}
