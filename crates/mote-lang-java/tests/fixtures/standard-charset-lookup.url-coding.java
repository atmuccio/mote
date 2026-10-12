// mote-test: params java_release=17
import java.net.URLDecoder;
import java.net.URLEncoder;

class Links {
    private static final String ENCODING = "UTF-8";
    private static final String OTHER = "windows-1252";
    private final String instanceEncoding = "UTF-8";

    String link(String q, String cs) throws Exception {
        String a = URLEncoder.encode(q, "UTF-8");                  // expect: standard-charset-lookup
        String b = URLDecoder.decode(q, ENCODING);                 // expect: standard-charset-lookup
        String c = java.net.URLEncoder.encode(q, "ISO-8859-1");    // expect: standard-charset-lookup

        String ok1 = URLEncoder.encode(q, OTHER);                  // not a standard charset
        String ok2 = URLEncoder.encode(q, cs);                     // a variable
        String ok3 = URLEncoder.encode(q, instanceEncoding);       // not a static constant
        String ok4 = URLEncoder.encode(q);                         // deprecated one-argument form
        String ok5 = Codec.encode(q, "UTF-8");                     // another class
        return a + b + c;
    }
}
