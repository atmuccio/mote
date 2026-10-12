import java.util.Date;

class Handler {
    void handle(Integer code) {}

    void at(Date when) {}

    void both(long a, String b) {}

    void generic(Object o) {}

    private void hidden(int x) {}

    static void util(int x) {}

    void exact(int x) {}

    void overloaded(int x) {}

    void overloaded(Integer x) {}
}

class RetryHandler extends Handler {
    void handle(int code) {}                                        // expect: overload-instead-of-override

    void at(java.sql.Date when) {}                                  // expect: overload-instead-of-override

    void both(Long a, String b) {}                                  // expect: overload-instead-of-override

    // ---- compliant --------------------------------------------------------------------

    void generic(String s) {}           // a different type: a deliberate overload

    void hidden(Integer x) {}           // private in the parent: nothing to override

    void util(Integer x) {}             // static in the parent

    void exact(int x) {}                // a real override

    void overloaded(int x) {}           // overrides one of the two

    @Override
    void handle(Integer code) {}
}

class Box<T> {
    void put(T value) {}
}

class IntBox extends Box<Integer> {
    void put(int value) {}              // generic parent: not considered
}

class External extends java.util.ArrayList<String> {
    public boolean add(String s) {
        return true;
    }
}
