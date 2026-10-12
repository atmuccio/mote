import static com.acme.util.Strings.isBlank;

import com.acme.util.Strings;

class Checker {
    // A method of this file with the imported name: the bare call could mean this one.
    static boolean isBlank(Object o) { return o == null; }

    boolean check(String s) {
        return Strings.isBlank(s) || isBlank((Object) s);
    }
}

class Other {
    // A different class with the same simple name is not the imported one.
    static class Strings {
        static boolean empty(String s) { return s.isEmpty(); }
    }

    boolean check(String s) {
        return Strings.empty(s);
    }
}
