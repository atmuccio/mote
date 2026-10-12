import java.util.List;

class StringCallWithKnownResult {
    private String sep = "/";

    String wrong(String path, String name, char c) {
        String a = path.replace("\\", "\\");                        // expect: string-call-with-known-result
        String b = path.replace('x', 'x');                          // expect: string-call-with-known-result
        String d = path.replace(sep, sep);                          // expect: string-call-with-known-result
        String e = path.replaceAll("abc", "abc");                   // expect: string-call-with-known-result
        String f = path.substring(0);                               // expect: string-call-with-known-result
        String g = path.substring(0, path.length());                // expect: string-call-with-known-result
        String h = name.concat("");                                 // expect: string-call-with-known-result
        String i = name.repeat(1);                                  // expect: string-call-with-known-result
        boolean j = name.startsWith("");                            // expect: string-call-with-known-result
        boolean k = "literal".contains("");                         // expect: string-call-with-known-result
        return a + b + d + e + f + g + h + i + j + k;
    }

    String fine(String path, String name, StringBuilder sb, Other other, List<String> list) {
        String a = path.replace("\\", "/");
        String b = path.replaceAll("a.c", "a.c");                    // regex: not the same text
        String c = path.replaceAll("$x", "$x");
        String d = path.substring(1);
        String e = path.substring(0, name.length());
        String f = name.concat("x");
        String g = name.repeat(2);
        boolean h = name.startsWith("a");
        String i = other.replace("a", "a");                         // not a String
        String j = path.replace(next(), next());                    // calls may differ
        path.substring(0);                                          // result discarded
        boolean k = list.contains("");
        String l = sb.substring(0);                                 // not a String
        String m = path.substring(0, path.length() - 1);
        return a + b + c + d + e + f + g + h + i + j + k + l + m;
    }

    String next() { return "x"; }

    interface Other { String replace(String a, String b); }
}
