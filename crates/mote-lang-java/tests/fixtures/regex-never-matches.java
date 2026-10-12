import java.util.List;
import java.util.regex.Pattern;
import static java.util.regex.Pattern.MULTILINE;

class RegexNeverMatches {
    static final Pattern CARET = Pattern.compile("^\\d+^");             // expect: regex-never-matches
    static final Pattern DOLLAR = Pattern.compile("\\d+$px");           // expect: regex-never-matches
    static final Pattern DOT_AFTER_END = Pattern.compile("end$.");      // expect: regex-never-matches
    static final Pattern WORDS = Pattern.compile("foo\\bbar");          // expect: regex-never-matches
    static final Pattern SPACES = Pattern.compile(", \\b,");            // expect: regex-never-matches
    static final Pattern NOT_BOUNDARY = Pattern.compile("a\\B ");       // expect: regex-never-matches
    static final Pattern AHEAD = Pattern.compile("(?=\\d)[a-z]+");      // expect: regex-never-matches
    static final Pattern NEG_AHEAD = Pattern.compile("(?!a)a");         // expect: regex-never-matches
    static final Pattern BEHIND = Pattern.compile("x(?<=y)");           // expect: regex-never-matches
    static final Pattern NO_GROUP = Pattern.compile("(\\w+)=\\2");      // expect: regex-never-matches
    static final Pattern LATER = Pattern.compile("\\1(a)");             // expect: regex-never-matches
    static final Pattern OTHER_BRANCH = Pattern.compile("(a)|b\\1");    // expect: regex-never-matches
    static final Pattern INPUT_START = Pattern.compile("a\\Ab");        // expect: regex-never-matches
    static final Pattern CI = Pattern.compile("(?i)id\\bx");            // expect: regex-never-matches
    static final Pattern CONCAT = Pattern.compile("^key" + "^value");  // expect: regex-never-matches
    static final Pattern MULTI = Pattern.compile("a^b", Pattern.MULTILINE); // expect: regex-never-matches

    boolean calls(String s, List<String> out) {
        boolean a = s.matches("\\w+$\\w+");                             // expect: regex-never-matches
        String b = s.replaceAll("x^", "");                              // expect: regex-never-matches
        String[] c = s.split(";$;");                                    // expect: regex-never-matches
        boolean d = Pattern.matches("(a)\\3", s);                       // expect: regex-never-matches
        boolean e = s.matches("\bword\b");                              // expect: regex-never-matches
        return a && d && e;
    }

    // Compliant: anchors and assertions where they can hold.
    static final Pattern OK1 = Pattern.compile("^\\d+$");
    static final Pattern OK2 = Pattern.compile("^$");
    static final Pattern OK3 = Pattern.compile("a*^b");
    static final Pattern OK4 = Pattern.compile("(?:,|^)value");
    static final Pattern OK5 = Pattern.compile("\\bword\\b");
    static final Pattern OK6 = Pattern.compile("line$\\r?\\n");
    static final Pattern OK7 = Pattern.compile("(?m)^\\s*$\\n^next");
    static final Pattern OK8 = Pattern.compile("\\n^x", MULTILINE);
    static final Pattern OK9 = Pattern.compile("(?=\\d)\\w+");
    static final Pattern OK10 = Pattern.compile("(?!^)");
    static final Pattern OK11 = Pattern.compile("(\\w)\\1");
    static final Pattern OK12 = Pattern.compile("(?:(a)|b\\1)+");
    static final Pattern OK13 = Pattern.compile("(?<=[a-z])(?=[A-Z])");
    static final Pattern OK14 = Pattern.compile("end$.", Pattern.DOTALL);
    static final Pattern OK15 = Pattern.compile("a\\$b\\^c");
    static final Pattern OK16 = Pattern.compile("[$^]x");
    static final Pattern OK17 = Pattern.compile("\\p{L}\\b\\p{L}");
    static final Pattern OK18 = Pattern.compile("(?x) a ^ b");

    // Possessive repetitions keep what they took.
    static final Pattern STARVED = Pattern.compile("\\d++\\d");            // expect: regex-never-matches
    static final Pattern STARVED_S = Pattern.compile("[a-z]*+s\\b");       // expect: regex-never-matches
    static final Pattern OK19 = Pattern.compile("color:\\s*+#([0-9a-fA-F]{6})");
    static final Pattern OK20 = Pattern.compile("\\d{1,3}+\\d");
    static final Pattern OK21 = Pattern.compile("[^.]++\\.txt");
    static final Pattern OK22 = Pattern.compile("\\d++");
    static final Pattern OK19 = Pattern.compile("a^b", Pattern.COMMENTS);
    static final Pattern OK20 = Pattern.compile("a^b", Pattern.LITERAL);
    static final Pattern OK21 = Pattern.compile("x\\Q^\\Ey");
    static final Pattern OK22 = Pattern.compile("a\\\\b");

    boolean fine(String s, String dynamic, int flags, Other other) {
        boolean a = s.matches("\\d+(\\.\\d+)?");
        Pattern p = Pattern.compile("a^b", flags);            // flags not constant
        Pattern q = Pattern.compile(dynamic + "^b");          // not a constant
        boolean r = other.matches("a^b");                     // not a String
        String t = s.replace("a^b", "");                       // not a regex
        String u = s.replaceAll("[\b]", "");                   // backspace inside a class
        boolean v = s.matches("(?<name>x)\\k<name>");
        return a && r && v;
    }

    interface Other { boolean matches(String s); }
}
