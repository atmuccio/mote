import java.util.Locale;

class Keys {
    private String name = "x";

    void m(String s, StringBuilder sb, Unknown u, char c) {
        String a = s.toLowerCase();                                 // expect: case-conversion-without-locale
        String b = name.toUpperCase();                              // expect: case-conversion-without-locale
        String d = "ABC".toLowerCase();                             // expect: case-conversion-without-locale
        String e = s.trim().toUpperCase();                          // expect: case-conversion-without-locale
        String f = (s + "x").toLowerCase();                         // expect: case-conversion-without-locale

        String ok1 = s.toLowerCase(Locale.ROOT);
        String ok2 = u.toLowerCase();                               // unknown type
        char ok3 = Character.toUpperCase(c);
        String ok4 = toUpperCase();                                 // own method
    }

    String toUpperCase() {
        return "";
    }
}
