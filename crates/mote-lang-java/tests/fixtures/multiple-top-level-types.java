// mote-test: path src/main/java/demo/Order.java
package demo;

public class Order {
    static class Line {
    }
}

class OrderLine {                                             // expect: multiple-top-level-types
}

interface OrderListener {                                     // expect: multiple-top-level-types
}

enum OrderState {                                             // expect: multiple-top-level-types
    OPEN, CLOSED
}

record OrderId(String value) {                                // expect: multiple-top-level-types
}
