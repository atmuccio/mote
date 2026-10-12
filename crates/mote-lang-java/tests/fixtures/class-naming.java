package com.acme;

class OrderLine {}
class order_line {}                      // expect: class-naming
class HTTPClient2 {}
interface httpHandler {}                 // expect: class-naming
enum Color_Mode { RED }                  // expect: class-naming
record point(int x) {}                   // expect: class-naming
@interface myAnnotation {}               // expect: class-naming
class Outer {
    static class Inner_Helper {}         // expect: class-naming
    class Fine {}
    void m() {
        class localThing {}              // expect: class-naming
        Runnable r = new Runnable() {    // anonymous classes have no name
            public void run() {}
        };
    }
}
class $Generated {}                      // expect: class-naming
