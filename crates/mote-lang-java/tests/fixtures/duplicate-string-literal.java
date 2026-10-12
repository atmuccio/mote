import java.util.*;

class Duplicates {
    void a(Map<String, Object> row) {
        row.get("customer_id");                              // expect: duplicate-string-literal
        row.put("customer_id", 1);
        log("customer_id");
        log("short");                                        // expect: duplicate-string-literal
        log("short");
        log("short");
        log("abcd");                                         // below min_length
        log("abcd");
        log("abcd");
        log("twice only");
        log("twice only");
        log("Customer_id");                                  // case matters
        String tb = """
            customer_id
            """;
    }

    @SuppressWarnings("unchecked")
    void b() {}
    @SuppressWarnings("unchecked")
    void c() {}
    @SuppressWarnings("unchecked")
    void d() {}
    @SuppressWarnings(value = {"rawtypes", "unchecked"})
    void e() {}

    void log(String s) {}

    @interface Doc {
        String value() default "undocumented";
        String other() default "undocumented";
        String third() default "undocumented";
    }

    // Separators and padded template lines are not hidden constants.
    String s1 = "---------", s2 = "---------", s3 = "---------";
    String t1 = "select a" +
        "        from t    " +
        "        from t    " +
        "        from t    ";

    String x = "héllo wörld";                                // expect: duplicate-string-literal
    String y = "héllo wörld";
    String z = "héllo wörld";
}
