import java.util.List;
import java.util.logging.Logger;

class Constants implements java.io.Serializable {
    private static final long serialVersionUID = 1L;
    static final int MAX_RETRIES = 3;
    static final int maxRetries = 3;                         // expect: constant-naming
    public static final String defaultName = "n/a";          // expect: constant-naming
    static final java.lang.String Prefix = "p";              // expect: constant-naming
    static final double PI_2 = 6.28, halfPi = 1.57;          // expect: constant-naming
    static final boolean DEBUG__MODE = false;                // expect: constant-naming
    static final char SEP = ',';
    static final String V2_KEY = "k";
    static final String _HIDDEN = "h";                       // expect: constant-naming
    static final int[] lookupTable = {1, 2};                 // arrays are not constants here
    static final int legacy[] = {1};
    static final List<String> names = List.of();             // not primitive/String
    static final Integer boxedValue = 5;                     // not primitive/String
    private static final Logger log = Logger.getLogger("x");
    static final String logger = "not really a logger";      // logger names are skipped
    static int counter = 0;                                  // not final
    final int instanceLimit = 5;                             // not static

    interface Api {
        int timeoutMillis = 100;                             // expect: constant-naming
        String NAME = "api";
    }

    enum Mode {
        ON;
        static final int defaultLevel = 1;                   // expect: constant-naming
    }

    void m() {
        final int localConstant = 1;                         // locals are not constants
        Object o = new Object() {
            static final int innerValue = 2;                 // expect: constant-naming
        };
    }
}

@interface Config {
    int maxSize = 10;                                        // expect: constant-naming
    int size() default 5;
}
