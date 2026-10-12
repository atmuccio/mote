import java.math.BigDecimal;
import java.math.BigInteger;

// Without a known Java release, TWO may not exist.
class UnknownRelease {
    BigInteger two = new BigInteger("2");
    BigInteger ten = new BigInteger("10");                           // expect: boxed-primitive-constructor
}
