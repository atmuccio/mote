import java.util.Properties;

class BoxingRoundtrip {
    int explicit(String text, Properties props) {
        int port = Integer.valueOf(text).intValue();                         // expect: boxing-roundtrip
        long id = Long.valueOf(props.getProperty("id")).longValue();         // expect: boxing-roundtrip
        double d = java.lang.Double.valueOf(text).doubleValue();             // expect: boxing-roundtrip
        int radix = Integer.valueOf(text, 16).intValue();                    // expect: boxing-roundtrip
        return port;
    }

    boolean implicit(String text, int n) {
        boolean flag = Boolean.valueOf(text);                                // expect: boxing-roundtrip
        int same = Integer.valueOf(n);                                       // expect: boxing-roundtrip
        return flag;
    }

    long returned(String text) {
        return Long.valueOf(text);                                           // expect: boxing-roundtrip
    }

    // Compliant
    Integer boxed(String text) {
        return Integer.valueOf(text);
    }

    long widened(String text) {
        long a = Integer.valueOf(text).longValue();                          // a different primitive
        Integer b = Integer.valueOf(text);
        Object c = Integer.valueOf(text);
        long d = Integer.valueOf(text);                                      // widening, not the same primitive
        int e = Integer.parseInt(text);
        int f = new Integer(text).intValue();                                // boxed-primitive-constructor's case
        Runnable r = () -> Integer.valueOf(text);
        return a + b + d + e + f;
    }

}
