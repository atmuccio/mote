import java.util.List;
import java.util.regex.Pattern;

class Codes {
    private static final String CODE_REGEX = "[A-Z]{3}-\\d{4}";
    private static final Pattern PRECOMPILED = Pattern.compile("[a-z]+");
    private static Pattern lazy;
    private Pattern perInstance;

    boolean positives(String code, List<String> lines) {
        boolean a = Pattern.compile("[A-Z]{3}-\\d{4}").matcher(code).matches();   // expect: regex-compiled-per-call
        Pattern p = Pattern.compile(CODE_REGEX, Pattern.CASE_INSENSITIVE);          // expect: regex-compiled-per-call
        boolean b = Pattern.matches("\\d+" + "x", code);                           // expect: regex-compiled-per-call
        for (String line : lines) {
            String[] cols = line.split("\\s*,\\s*");                               // expect: regex-compiled-per-call
            String clean = line.replaceAll("[^a-z]", "");                           // expect: regex-compiled-per-call
            if (line.matches("\\d+")) {                                             // expect: regex-compiled-per-call
                return true;
            }
        }
        int i = 0;
        while (i < lines.size()) {
            lines.get(i).split("\\.\\.");                                            // expect: regex-compiled-per-call
            i++;
        }
        lines.forEach(l -> Pattern.compile("x+").matcher(l).find());               // expect: regex-compiled-per-call
        return a || b || p != null;
    }

    void negatives(String code, List<String> lines, String dynamic, int flags) {
        Pattern built = Pattern.compile(dynamic);                                  // built at run time
        Pattern flagged = Pattern.compile("x+", flags);                            // run-time flags
        if (lazy == null) {
            lazy = Pattern.compile("[0-9]+");                                      // lazy initialization
        }
        this.perInstance = Pattern.compile("[0-9]+");                              // stored in a field
        String[] parts = code.split("\\s+");                                       // not in a loop
        for (String line : lines) {
            String[] cols = line.split(",");                                       // fast path
            String[] bars = line.split("\\|");                                     // fast path
            String plain = line.replaceAll("abc", "x");                            // no regex syntax
            boolean ok = PRECOMPILED.matcher(line).matches();
            String[] tabs = line.split("\t");                                      // fast path
        }
        for (String part : code.split("[,;]")) {                                   // evaluated once
            System.out.println(part);
        }
        for (int i = 0, n = code.split("[,;]").length; i < n; i++) {               // init runs once
            System.out.println(i);
        }
    }

    Codes() {
        Pattern p = Pattern.compile("[a-z]+");                                     // constructor
    }

    static {
        Pattern p = Pattern.compile("[a-z]+");                                     // static initializer
    }

    private final Pattern field = Pattern.compile("[a-z]+");                       // field initializer

    public static void main(String[] args) {
        Pattern p = Pattern.compile("[a-z]+");                                     // main runs once
    }
}
