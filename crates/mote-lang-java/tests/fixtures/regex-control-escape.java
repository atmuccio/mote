import java.util.regex.Pattern;

class ControlEscapes {
    static final Pattern BELL = Pattern.compile("\\cg");                    // expect: regex-control-escape
    static final Pattern DIGIT = Pattern.compile("x\\c1y");                 // expect: regex-control-escape
    static final Pattern IN_CLASS = Pattern.compile("[\\cm\\cj]+");         // expect: regex-control-escape

    String strip(String s) {
        return s.replaceAll("\\ch", "");                                    // expect: regex-control-escape
    }

    // Compliant
    static final Pattern OK1 = Pattern.compile("\\cG");
    static final Pattern OK2 = Pattern.compile("[\\cM\\cJ]+");
    static final Pattern OK3 = Pattern.compile("\\c@\\c[\\c_\\c?");
    static final Pattern OK4 = Pattern.compile("\\\\ca");                  // an escaped backslash, then "ca"
    static final Pattern OK5 = Pattern.compile("\\Q\\ca\\E");               // quoted literal text
    static final Pattern OK6 = Pattern.compile("\\x07");
    static final String NOT_A_REGEX = "C:\\cache";
}
