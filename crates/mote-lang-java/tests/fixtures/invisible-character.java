class Invisible {
    String zw = "a​b";                                   // expect: invisible-character
    String nbsp = "10 EUR";                              // expect: invisible-character
    String twice = " x ";                           // expect: invisible-character
    String bidi = "user‮";                                // expect: invisible-character
    // comment with a bidi override ‮ inside              // expect: invisible-character
    // comment with a no-break space is fine
    String ctl = "ab";                                   // expect: invisible-character
    String escaped = "10\u00A0EUR";
    char quote = '\u0022';
    String accents = "café naïve";
    int \u0061bc = 1;                                       // expect: invisible-character
    // a line break escape \u000a in a comment              // expect: invisible-character
    // an ordinary escape \u00e9 in a comment
    String backslash = "\\u0041";
    int spaced = 2;                                       // expect: invisible-character
}
