class Invoice {
    private String Number;
    private String number;                                         // expect: confusing-member-name
    private static final String NAME = "name";
    private String name;                                           // constant + field: fine

    public void Invoice() {                                        // expect: confusing-member-name
        number = "0";
    }

    public Invoice(String n) {
        number = n;
    }

    String getUrl() { return ""; }
    String getURL() { return ""; }                                 // expect: confusing-member-name

    void reset() { }
    void reset(int n) { }                                          // overload: fine

    String name() { return name; }                                 // field + accessor: fine

    @Deprecated
    String getID() { return ""; }
    String getId() { return ""; }                                  // deprecated alias: fine

    void setUp() { }
    void setup() { }                                               // expect: confusing-member-name
    void SETUP() { }                                               // expect: confusing-member-name
}

enum Level {
    LOW, HIGH;

    int value;
    int Value;                                                     // expect: confusing-member-name
}

interface Api {
    String KEY = "k";
    String key = "k2";                                             // interface fields are constants: fine
}

record Point(int x, int y) {
    int Point() { return 0; }                                      // expect: confusing-member-name
}

class Outer {
    int count;

    class Inner {
        int Count;                                                 // other class: fine
    }
}

class Builder {
    protected static Object FACTORY;
    protected Object factory;                                      // static capitals + field: fine
}

class Response implements javax.servlet.http.HttpServletResponse {
    @Override
    public String encodeURL(String s) { return s; }

    @Override
    public String encodeUrl(String s) { return s; }                 // imposed by the interface: fine
}
