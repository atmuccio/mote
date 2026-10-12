import java.util.regex.Pattern;

class RegexEmptyAlternative {
    String[] split(String line) {
        String[] a = line.split(",|;|");                               // expect: regex-empty-alternative
        String[] b = line.split("|,");                                 // expect: regex-empty-alternative
        String[] c = line.split("(,|;|)");                             // expect: regex-empty-alternative
        String d = line.replaceAll("x|", "-");                         // expect: regex-empty-alternative
        String e = line.replaceFirst("|x", "-");                       // expect: regex-empty-alternative
        boolean f = line.matches("(cat||dog)s?");                      // expect: regex-empty-alternative
        String[] g = Pattern.compile("a|").split(line);                // expect: regex-empty-alternative
        boolean h = Pattern.compile("a|").matcher(line).find();        // expect: regex-empty-alternative
        return a;
    }

    @javax.validation.constraints.Pattern(regexp = "x||y")             // expect: regex-empty-alternative
    String code;

    // Compliant
    String[] fine(String line) {
        String[] a = line.split(",|;");
        String b = line.replaceAll("x|", "");                  // empty replacement: no effect
        boolean c = line.matches("\\d+|");                     // whole match: "digits or nothing"
        Pattern d = Pattern.compile("a|");                     // usage unknown
        String[] e = line.split("a(b|)");                      // optional part inside a group
        String[] f = line.split("(|a)b");
        boolean g = Pattern.compile("a|").matcher(line).matches();
        String h = line.replaceAll("\\|\\|", "or");
        String i = line.replaceAll("[||]", "or");
        String[] j = line.split("");
        return a;
    }
}
