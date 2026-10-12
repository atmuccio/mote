import java.util.regex.Pattern;

class RegexShadowedAlternative {
    static final Pattern EXT = Pattern.compile("\\.(jpg|png|jpg)$");   // expect: regex-shadowed-alternative
    static final Pattern WORDS = Pattern.compile("(?:cat|dog|cat)s");  // expect: regex-shadowed-alternative

    String[] parts(String line) {
        String[] a = line.split(",|, ");                               // expect: regex-shadowed-alternative
        String[] b = line.split(";|;\\s");                             // expect: regex-shadowed-alternative
        String c = line.replaceAll("(?:Mr|Mrs)\\.?\\s(a|an)", "x");    // expect: regex-shadowed-alternative
        String d = line.replaceFirst("a|ab", "x");                     // expect: regex-shadowed-alternative
        boolean e = line.matches("yes|no|yes");                        // expect: regex-shadowed-alternative
        String[] f = Pattern.compile("-|->").split(line);              // expect: regex-shadowed-alternative
        return a;
    }

    // Compliant
    String[] fine(String line) {
        String[] a = line.split(", |,");
        boolean b = line.matches(",|, ");                      // whole match backtracks into ", "
        Pattern c = Pattern.compile("a|ab");                   // usage unknown
        String d = line.replaceAll("(a|ab)c", "x");            // something follows the alternation
        String e = line.replaceAll("(a|ab)$", "x");
        String f = line.replaceAll("(a)|(a)", "x");            // capturing groups differ
        String g = line.replaceAll("a|x(?i)|a", "y");          // inline flags change later alternatives
        String h = line.replaceAll("ab|a", "x");
        boolean i = Pattern.compile("a|ab").matcher(line).matches();
        String j = line.replaceAll("[a|a]", "x");
        return a;
    }
}
