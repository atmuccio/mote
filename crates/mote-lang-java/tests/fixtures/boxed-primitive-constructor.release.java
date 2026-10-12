// mote-test: params java_release=21
import java.math.BigDecimal;
import java.math.BigInteger;

class Twos {
    BigDecimal decimal = new BigDecimal(2);                          // expect: boxed-primitive-constructor
    BigInteger integer = new BigInteger("2");                        // expect: boxed-primitive-constructor
    BigDecimal three = new BigDecimal(3);
}
