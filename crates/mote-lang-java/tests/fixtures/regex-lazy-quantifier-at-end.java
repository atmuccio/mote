import java.util.regex.Pattern;

class LazyEnds {
    String clean(String html, String line) {
        String a = html.replaceAll("<!--.*?", "");                         // expect: regex-lazy-quantifier-at-end
        String b = line.replaceFirst("key=\\w+?", "key=*");                 // expect: regex-lazy-quantifier-at-end
        String[] c = line.split(",\\s*?");                                  // expect: regex-lazy-quantifier-at-end
        boolean d = Pattern.compile("id:(\\d{2,6}?)").matcher(line).find();   // expect: regex-lazy-quantifier-at-end
        String e = line.replaceAll("a|b+?", "-");                           // expect: regex-lazy-quantifier-at-end
        return a + b + c.length + d + e;
    }

    // Compliant
    static final Pattern MEDIA = Pattern.compile("/ppt/media/.*?");        // use not visible here

    boolean ok(String html, String line) {
        String a = html.replaceAll("<!--.*?-->", "");
        boolean b = line.matches(".*?COMPANY.*?");                          // matched as a whole
        String c = line.replaceAll("x.*?$", "");
        String d = line.replaceAll("a{2}?", "");                            // a fixed count
        String e = line.replaceAll("a+?\\b", "");
        String f = line.replaceAll("\\?", "");
        String g = line.replaceAll(".*", "");
        return b && (a + c + d + e + f + g).isEmpty();
    }
}
