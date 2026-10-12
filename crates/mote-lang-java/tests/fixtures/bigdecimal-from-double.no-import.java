import com.acme.money.*;

class NoImport {
    Object make(double d) {
        // `BigDecimal` could come from the wildcard import: the type is unknown.
        return new BigDecimal(0.1);
    }

    Object qualified(double d) {
        return new java.math.BigDecimal(d); // expect: bigdecimal-from-double
    }
}
