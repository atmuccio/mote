import java.util.regex.Pattern;
import javax.validation.constraints.Email;

class Patterns {
    static final String OCTET = "(?:25[0-5]|2[0-4]\\d|1\\d\\d|[1-9]?\\d)";

    // Nested alternations of quantified groups.
    static final Pattern IP = Pattern.compile(                     // next line
        "^(?:(?:25[0-5]|2[0-4]\\d|1\\d\\d|[1-9]?\\d)\\.){3}(?:25[0-5]|2[0-4]\\d|1\\d\\d|[1-9]?\\d)$"); // expect: regex-too-complex

    // Built from a constant: still the same total.
    static final Pattern IP2 = Pattern.compile("^" + OCTET + "(?:\\." + OCTET + "){3}$"); // expect: regex-too-complex

    boolean deep(String s) {
        return s.matches(                                           // next line
            "^(?:(\\d+)(?:\\.(\\d+))*|[a-f]+(?:-[a-f]+)*)(?:\\s*,\\s*(?:(\\d+)(?:\\.(\\d+))*|[a-f]+(?:-[a-f]+)*))*$"); // expect: regex-too-complex
    }

    // Fine: simple, long but flat, comments mode, runtime patterns.
    static final Pattern DATE = Pattern.compile("\\d{4}-\\d{2}-\\d{2}");
    static final Pattern VERBS = Pattern.compile("^(GET|PUT|POST|DELETE|PATCH|HEAD|OPTIONS|TRACE|CONNECT) (/[a-z]*)+$");
    static final Pattern WORDS = Pattern.compile("the quick brown fox jumps over the lazy dog and keeps running");
    static final Pattern EMAIL = Pattern.compile("[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\\.[a-zA-Z]{2,}");
    static final Pattern COMMENTED = Pattern.compile(
        "^(?:(?:25[0-5]|2[0-4]\\d|1\\d\\d|[1-9]?\\d)\\.){3}(?:25[0-5]|2[0-4]\\d|1\\d\\d|[1-9]?\\d)$", Pattern.COMMENTS);

    boolean runtime(String s, String p) {
        return s.matches(p);
    }

    String split(String s) {
        return s.replaceAll("\\s+", " ");
    }
}

class Bean {
    @javax.validation.constraints.Pattern(regexp = "^(?:(?:25[0-5]|2[0-4]\\d|1\\d\\d|[1-9]?\\d)\\.){3}(?:25[0-5]|2[0-4]\\d|1\\d\\d|[1-9]?\\d)$") // expect: regex-too-complex
    String ip;
}
