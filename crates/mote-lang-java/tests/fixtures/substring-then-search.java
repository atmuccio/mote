class SubstringThenSearch {
    int parse(String line, int pos) {
        if (line.substring(pos).startsWith("--")) {                          // expect: substring-then-search
            return -1;
        }
        int end = line.substring(pos).indexOf(';');                          // expect: substring-then-search
        int dot = line.trim().substring(pos + 1).indexOf(".");               // expect: substring-then-search
        return end + dot;
    }

    // Compliant
    int fine(String line, int pos, StringBuilder sb, Object unknown) {
        boolean a = line.startsWith("--", pos);
        int b = line.indexOf(';', pos);
        int c = line.substring(pos, pos + 5).indexOf('x');                   // bounded substring
        int d = line.substring(pos).lastIndexOf('x');                        // no equivalent overload
        boolean e = line.substring(pos).contains("x");
        boolean f = line.substring(pos).endsWith("x");
        int g = line.substring(pos).indexOf("x", 2);                        // already an offset
        int h = sb.substring(pos).indexOf("x");                              // not a String receiver
        String rest = line.substring(pos);
        int i = rest.indexOf('x');
        return b;
    }

    static class Text {
        Text substring(int k) { return this; }
        int indexOf(String s) { return 0; }
    }

    int custom(Text t) {
        return t.substring(1).indexOf("x");
    }
}
