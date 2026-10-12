import java.util.ArrayList;
import java.util.List;

class Labels {
    String label(String prefix, String name) {
        String label = prefix + name;                               // expect: immediately-returned-variable
        return label;
    }

    void fail() {
        IllegalStateException error = new IllegalStateException("closed"); // expect: immediately-returned-variable
        throw error;
    }

    List<String> list() {
        var result = new ArrayList<String>();                       // expect: immediately-returned-variable
        // a comment in between is fine
        return (result);
    }

    int count(int x) {
        int n = x * 2;                                              // expect: immediately-returned-variable
        return n;
    }

    @SuppressWarnings("unchecked")
    List<String> cast(Object raw) {
        @SuppressWarnings("unchecked")
        List<String> typed = (List<String>) raw;                    // annotated: keep
        return typed;
    }

    int[] array() {
        int[] values = {1, 2};                                      // array initializer
        return values;
    }

    Runnable runnable() {
        Runnable r = () -> { };                                     // lambda needs a target type
        return r;
    }

    Object widened(int x) {
        long wide = x;                                              // conversion changes the boxed type
        return wide;
    }

    String used(String name) {
        String s = name.trim();
        log(s);
        return s;
    }

    String other(String a) {
        String s = a;
        return a;
    }

    String two(String a) {
        String s = a, t = a;
        return s;
    }

    void log(String s) { }
}
