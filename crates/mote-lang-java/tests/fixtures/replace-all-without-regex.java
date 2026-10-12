class Csv {
    String positives(String line, StringBuilder sb) {
        String a = line.replaceAll(";", ",");                       // expect: replace-all-without-regex
        String b = line.replaceAll("foo", "");                      // expect: replace-all-without-regex
        String c = "a-b".replaceAll("-", "_");                      // expect: replace-all-without-regex
        String d = sb.toString().replaceAll("\t", " ");             // expect: replace-all-without-regex
        return a + b + c + d;
    }

    String negatives(String line, String sep, Other o) {
        String a = line.replaceAll("\\.", ",");                     // escaped metacharacter
        String b = line.replaceAll(".", ",");
        String c = line.replaceAll("a+", "b");
        String d = line.replaceAll(";", "$");                       // `$` in the replacement matters
        String e = line.replaceAll(";", sep);                       // non-literal replacement
        String f = line.replaceAll(sep, ";");
        String g = line.replaceAll("", "x");
        String h = line.replace(";", ",");
        String i = o.replaceAll(";", ",");                          // unknown type
        String j = line.replaceFirst(";", ",");
        return a + b + c + d + e + f + g + h + i + j;
    }
}
