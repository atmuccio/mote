import java.util.regex.Matcher;
import java.util.regex.Pattern;

class Templates {
    private static final Pattern BAD = Pattern.compile("(\\d+");            // expect: invalid-regex-literal

    String positives(String template, String name, String line) {
        String a = template.replaceAll("{name}", name);                     // expect: invalid-regex-literal
        String b = template.replaceAll("${user}", name);                    // expect: invalid-regex-literal
        String[] c = line.split("*");                                       // expect: invalid-regex-literal
        String[] d = line.split("\\");                                      // expect: invalid-regex-literal
        boolean e = line.matches("[a-z");                                   // expect: invalid-regex-literal
        boolean f = Pattern.matches("a)b", line);                           // expect: invalid-regex-literal
        String g = line.replaceFirst("+x", "");                             // expect: invalid-regex-literal
        Pattern h = java.util.regex.Pattern.compile("[z-a]");               // expect: invalid-regex-literal
        boolean i = "abc".matches("(a|*b)");                                // expect: invalid-regex-literal
        return a + b + c + d + e + f + g + h + i;
    }

    String negatives(String line, Matcher m, Splitter s, String pattern) {
        String a = line.replaceAll("\\{name\\}", "x");
        String b = line.replaceAll("\\s+", " ");
        String[] c = line.split("\\|");
        String[] d = line.split("|");
        boolean e = line.matches("[]a-z]+");
        Pattern f = Pattern.compile("(?i)abc(?<word>\\w+)");
        Pattern g = Pattern.compile("(a", Pattern.LITERAL);                 // flags change the syntax
        Pattern h = Pattern.compile("\\Q(\\E");
        String i = m.replaceAll("$1");                                      // a replacement, not a regex
        String[] j = s.split("(");                                          // unknown type
        String k = line.replaceAll(pattern, "x");                           // not a literal
        Pattern l = Pattern.compile("[a[b]]");                              // nested class: not modelled
        String n = line.replaceAll("x{2,3}", "y");
        Pattern o = Pattern.compile("\\p{Alpha}+\\x{41}");
        String p = line.replaceAll("\\\\", "/");
        return a + b + c + d + e + f + g + h + i + j + k + l + n + o + p;
    }
}
