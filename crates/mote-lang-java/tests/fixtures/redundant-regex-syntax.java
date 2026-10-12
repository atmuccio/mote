import java.util.regex.Matcher;
import java.util.regex.Pattern;

class RedundantRegexSyntax {
    private static final Pattern ONCE = Pattern.compile("[A-Z]{1}\\d+");      // expect: redundant-regex-syntax
    private static final Pattern STAR = Pattern.compile("x{0,}y");           // expect: redundant-regex-syntax
    private static final Pattern PLUS = Pattern.compile("(ab){1,}");         // expect: redundant-regex-syntax
    private static final Pattern OPT = Pattern.compile("-{0,1}\\d");         // expect: redundant-regex-syntax
    private static final Pattern LAZY = Pattern.compile("<.{0,}?>");         // expect: redundant-regex-syntax

    boolean shapes(String s) {
        boolean a = s.matches("a(?:)b");                                      // expect: redundant-regex-syntax
        boolean b = s.matches("a()b");                                        // expect: redundant-regex-syntax
        String[] c = s.split("[,]");                                          // expect: redundant-regex-syntax
        String[] d = s.split("[\\s]+");                                       // expect: redundant-regex-syntax
        boolean e = s.matches("[\\w_]+");                                     // expect: redundant-regex-syntax
        boolean f = s.matches("[a-z0-9a]+");                                  // expect: redundant-regex-syntax
        boolean g = s.matches("[\\s\\n]*");                                   // expect: redundant-regex-syntax
        String[] h = s.split(",|;|:");                                        // expect: redundant-regex-syntax
        String[] i = s.split("\\s|,");                                        // expect: redundant-regex-syntax
        boolean j = s.matches("(a|b)c");                                      // expect: redundant-regex-syntax
        String[] k = s.split("   ");                                          // expect: redundant-regex-syntax
        boolean l = s.matches("(?<year>\\d{4})-\\d{2}");                      // expect: redundant-regex-syntax
        String m = s.replaceAll("(?<word>\\w+)!", "$1?");                     // expect: redundant-regex-syntax
        String n = s.replaceAll("(?<w>x)", "<$0>");                           // expect: redundant-regex-syntax
        boolean o = s.matches("[^aa]");                                       // expect: redundant-regex-syntax
        return a;
    }

    @javax.validation.constraints.Pattern(regexp = "[0-9]{1}[a-z]+")          // expect: redundant-regex-syntax
    String code;

    // Compliant
    boolean fine(String s) {
        boolean a = s.matches("[A-Z]\\d{2}");                 // an exact count other than one
        boolean b = s.matches("x{2,}y{0,3}");                 // ranges without a symbol
        boolean c = s.matches("a{0}b");                       // not a long form of anything
        String[] d = s.split("[.]");                          // a class quoting a metacharacter
        String[] e = s.split("[ ]");                          // a class making a space visible
        String[] f = s.split("[$]");
        String[] g = s.split("[\\.]");                        // escaped already: left alone
        boolean h = s.matches("[^a]");                        // negated: means something else
        boolean i = s.matches("[a-zA-Z0-9_-]+");
        boolean j = s.matches("[\\s\\S]*");                   // the "any character" idiom
        boolean k = s.matches("[\\p{L}\\p{M}]+");             // unknown sets
        boolean l = s.matches("(?i)[aA]");                    // only equal when ignoring case
        String[] m = s.split("[jpg|png|gif]");                // the suspicious-class rule's case
        String[] n = s.split("\\r\\n|\\n");                   // a branch of two characters
        String[] o = s.split(".|\\n");                        // `.` means something else in a class
        String[] p = s.split("a|a");                          // duplicate branch: another rule
        boolean q = s.matches("(?!)");                        // an empty lookaround never matches
        String[] r = s.split(" ");
        String[] t = s.split(" +");
        String[] u = s.split("\\x20\\x20");                   // escapes, not literal spaces
        String[] v = s.split("\\Qa  b\\E");                   // quoted text
        String w = s.replaceAll("(?<word>\\w+)!", "${word}?"); // the replacement uses the name
        boolean x = s.matches("(?<q>['\"]).*\\k<q>");          // back-reference by name
        String y = s.replaceAll("()x", "$1");                 // groups read by the replacement
        boolean z = s.matches(s + "{1}");                     // not a constant
        return a;
    }

    Matcher named(String s) {
        Pattern p = Pattern.compile("(?<key>\\w+)=(?<value>\\w*)"); // the matcher may read the names
        Pattern q = Pattern.compile("a()b");                  // groups readable through a matcher
        Pattern r = Pattern.compile("(?x) a {1}");            // comments mode: not modelled
        return p.matcher(s);
    }

    boolean notRegex(String s, StringBuilder sb) {
        return s.contains("{1}") || s.replace("[,]", ";").isEmpty() || s.startsWith("a|b");
    }
}
