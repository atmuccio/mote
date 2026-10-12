// mote-test: test-file
class EqualityTest {
    void distinctInstances() {
        String a = new String("x");
        java.math.BigDecimal one = new java.math.BigDecimal(1);    // expect: boxed-primitive-constructor
    }
}
