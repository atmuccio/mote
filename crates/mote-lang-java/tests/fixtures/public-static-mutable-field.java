import org.junit.ClassRule;
import org.junit.rules.TemporaryFolder;

public class Globals {
    public static int timeoutMillis = 5000;                  // expect: public-static-mutable-field
    public static String a, b;                               // expect: public-static-mutable-field, public-static-mutable-field
    public static volatile boolean running;                  // expect: public-static-mutable-field
    public static final int LIMIT = 10;
    static int packagePrivate;
    private static int hidden;
    protected static int prot;
    public int instance;
    @ClassRule
    public static TemporaryFolder folder = new TemporaryFolder();

    public static class Nested {
        public static int counter;                           // expect: public-static-mutable-field
    }

    private static class Hidden {
        public static int notVisible;
    }

    interface Api {
        int CONSTANT = 1;                                    // implicitly final
    }

    enum Mode {
        A;
        public static int modeCount;                         // expect: public-static-mutable-field
    }

    Object anon = new Object() {
        public static int anonStatic;
    };
}
