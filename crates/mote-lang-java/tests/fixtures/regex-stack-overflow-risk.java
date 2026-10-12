import java.util.regex.Pattern;

class RegexStackOverflowRisk {
    static final Pattern BLOCK = Pattern.compile("BEGIN(.|\\n)*END");  // expect: regex-stack-overflow-risk
    static final Pattern LAZY = Pattern.compile("<!--(.|\\s)*?-->");   // expect: regex-stack-overflow-risk
    static final Pattern CODE = Pattern.compile("(?:\\d|-)+");         // expect: regex-stack-overflow-risk
    static final Pattern NAME = Pattern.compile("([a-z]|_){2,}");      // expect: regex-stack-overflow-risk

    boolean check(String s) {
        return s.matches("(a|b|c)+");                                  // expect: regex-stack-overflow-risk
    }

    // Compliant
    static final Pattern OK1 = Pattern.compile("BEGIN.*END", Pattern.DOTALL);
    static final Pattern OK2 = Pattern.compile("[\\d-]+");
    static final Pattern OK3 = Pattern.compile("(a|b)*+");             // possessive
    static final Pattern OK4 = Pattern.compile("(a|b){1,5}");          // bounded
    static final Pattern OK5 = Pattern.compile("(ab|cd)*");            // longer alternatives
    static final Pattern OK6 = Pattern.compile("(\\r\\n|\\n)*");
    static final Pattern OK7 = Pattern.compile("a|b*");
    static final Pattern OK8 = Pattern.compile("(?=a|b)*x");
}
