import java.util.regex.Pattern;
import static java.util.regex.Pattern.CASE_INSENSITIVE;
import static java.util.regex.Pattern.UNICODE_CASE;

class RegexCaseInsensitiveNonAscii {
    static final Pattern CITY = Pattern.compile("^münchen$", Pattern.CASE_INSENSITIVE);  // expect: regex-case-insensitive-non-ascii
    static final Pattern INLINE = Pattern.compile("(?i)café");                           // expect: regex-case-insensitive-non-ascii
    static final Pattern CYRILLIC = Pattern.compile("(?i)[а-я]+");                       // expect: regex-case-insensitive-non-ascii
    static final Pattern STATIC_IMPORT = Pattern.compile("Ärger", CASE_INSENSITIVE);     // expect: regex-case-insensitive-non-ascii

    boolean check(String s) {
        return s.matches("(?i)(?:oui|très bien)");                                     // expect: regex-case-insensitive-non-ascii
    }

    // Compliant
    static final Pattern OK1 = Pattern.compile("^münchen$", Pattern.CASE_INSENSITIVE | Pattern.UNICODE_CASE);
    static final Pattern OK2 = Pattern.compile("(?iu)café");
    static final Pattern OK3 = Pattern.compile("café");
    static final Pattern OK4 = Pattern.compile("(?i)[äÄ]");
    static final Pattern OK5 = Pattern.compile("(?i)straße");
    static final Pattern OK6 = Pattern.compile("(?i)hello");
    static final Pattern OK7 = Pattern.compile("(?i)[^é]");
    static final Pattern OK8 = Pattern.compile("Ärger", CASE_INSENSITIVE | UNICODE_CASE);
    static final Pattern OK9 = Pattern.compile("(?U)(?i)café");
    static final Pattern OK10 = Pattern.compile("(?i:abc)café");
}
