class NumericEscapeForNamedChar {
    String row(String name, String value) {
        return name + "\11" + value + "\12";                  // expect: numeric-escape-for-named-char, numeric-escape-for-named-char
    }

    void escapes() {
        char tab = '\u0009';                                  // expect: numeric-escape-for-named-char
        char backspace = '\10';                               // expect: numeric-escape-for-named-char
        String crlf = "\015\012";                             // expect: numeric-escape-for-named-char, numeric-escape-for-named-char
        String quote = "say \42hi\42";                        // expect: numeric-escape-for-named-char, numeric-escape-for-named-char
        String slash = "a\134b";                              // expect: numeric-escape-for-named-char
        String feed = "\u000c";                               // expect: numeric-escape-for-named-char
        String apostrophe = "it\47s";                         // expect: numeric-escape-for-named-char
        // In the text block, the harness reads the marker on the line starting with `*`.
        String block = """
            * a\11b expect: numeric-escape-for-named-char
            """;
    }

    void compliant() {
        String named = "\t\n\r\b\f\"\'\\";
        char nul = '\0';
        String esc = "\033[0m";
        String unicode = "\u001b";
        String space = "\40";
        String backslashDigits = "\\12";
        String regex = "(a)\\1";
        String high = "é";
        char quote = '"';
        char plain = 'a';
    }
}
