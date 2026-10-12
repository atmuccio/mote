import java.util.Map;
import java.util.logging.Logger;
import lombok.Getter;

class Fields {
    private int itemCount;
    private int Item_Count;                                  // expect: field-naming
    private String _name;                                    // expect: field-naming
    protected int m_value;                                   // expect: field-naming
    int MAX;                                                 // expect: field-naming
    final int LIMIT = 5;                                     // expect: field-naming
    static int instanceCounter;
    static int GLOBAL_COUNT;                                 // expect: field-naming
    int a, b_c, d;                                           // expect: field-naming
    private int x;
    public long parseHTTP2Count;
    @Getter private int Lombok_Field;                        // expect: field-naming

    // static final fields are constants or loggers: not this rule's business.
    private static final Logger LOG = Logger.getLogger("x");
    private static final Logger log = Logger.getLogger("y");
    static final Map<String, String> CACHE = Map.of();
    private static final long serialVersionUID = 1L;

    interface Api {
        int TIMEOUT = 5;                                     // interface fields are constants
    }

    record Point(int X, int y) {}                            // record components: not fields here

    enum Kind {
        A;
        private int Weight;                                  // expect: field-naming
    }

    Object anon = new Object() {
        int Hidden_Field;                                    // expect: field-naming
    };
}
