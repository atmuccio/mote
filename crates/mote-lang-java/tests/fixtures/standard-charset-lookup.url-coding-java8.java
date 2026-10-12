// mote-test: params java_release=8
import java.net.URLEncoder;

class Links {
    String link(String q) throws Exception {
        return URLEncoder.encode(q, "UTF-8");                      // no Charset overload before Java 10
    }
}
