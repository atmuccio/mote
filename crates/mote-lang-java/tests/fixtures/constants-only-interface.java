public interface Limits {                                          // expect: constants-only-interface
    int MAX_USERS = 100;
    String NAME = "limits";
}

interface WithMethod {
    int MAX = 1;

    void run();
}

interface Marker {
}

interface Extending extends Marker {
    int VALUE = 1;
}

interface WithNested {
    int VALUE = 1;

    class Helper { }
}

interface OnlyNested {
    enum Kind { A, B }
}

@interface Config {
    int DEFAULT = 1;
}

class Holder {
    interface Codes {                                               // expect: constants-only-interface
        // comments are fine
        int OK = 0;
        int FAILED = 1;
    }
}
