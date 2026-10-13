// Numeric literals with repeated underscores, escapes, and text blocks with line continuations.
class Literals {
    int million = 1__000__000;
    long big = 9_223__372L;
    int octal = 0__7;
    int hex = 0xFF__FF;
    int bits = 0b1010__0101;
    double ratio = 1__0.5__0;
    double exponent = 1e1__0;
    float pi = 3__1.4__1f;
    char letter = '\u0041';
    String escapes = "tab\tnew\nline \s end \0";
    String emoji = "smile 😃 done";
    String block = """
        one \
        two
        """;
}
