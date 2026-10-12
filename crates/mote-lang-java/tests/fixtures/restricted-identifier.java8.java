// mote-test: params java_release=8
import java.util.function.BiFunction;
import java.util.function.IntUnaryOperator;

class Legacy {
    void m(int _) {                                                 // expect: restricted-identifier
        IntUnaryOperator f = _ -> 1;                                // expect: restricted-identifier
        BiFunction<Integer, Integer, Integer> g = (_, y) -> y;      // expect: restricted-identifier
        try {
            m(1);
        } catch (RuntimeException _) {                              // expect: restricted-identifier
        }
    }
}

class sealed {                                                      // expect: restricted-identifier
}
