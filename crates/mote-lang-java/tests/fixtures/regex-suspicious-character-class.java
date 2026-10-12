import java.util.regex.Pattern;

class RegexSuspiciousCharacterClass {
    static final Pattern NAME = Pattern.compile("[A-z0-9]+");               // expect: regex-suspicious-character-class
    static final Pattern HEX = Pattern.compile("[0-9A-f]+");                // expect: regex-suspicious-character-class
    static final Pattern ALNUM = Pattern.compile("[0-z]");                  // expect: regex-suspicious-character-class
    static final Pattern IMAGES = Pattern.compile(".*\\.[jpg|png]");       // expect: regex-suspicious-character-class
    static final Pattern CODES = Pattern.compile("[x|y|z]");                // expect: regex-suspicious-character-class
    static final Pattern ACCENT = Pattern.compile("[é]");             // expect: regex-suspicious-character-class

    String clean(String s) {
        return s.replaceAll("[;|,|:]", " ");                                 // expect: regex-suspicious-character-class
    }

    // Compliant
    static final Pattern OK1 = Pattern.compile("[A-Za-z0-9]+");
    static final Pattern OK2 = Pattern.compile("[!-~]");
    static final Pattern OK3 = Pattern.compile("[|,;]");
    static final Pattern OK4 = Pattern.compile("[a-z&&[^aeiou]]");
    static final Pattern OK5 = Pattern.compile("(?:jpg|png)");
    static final Pattern OK6 = Pattern.compile("[\\w|]");
    static final Pattern OK7 = Pattern.compile("[0-9a-fA-F]");
    static final Pattern OK8 = Pattern.compile("\\[A-z\\]");
    static final Pattern OK9 = Pattern.compile("[||]");
}
