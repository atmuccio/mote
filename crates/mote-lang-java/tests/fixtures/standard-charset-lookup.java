import java.io.InputStream;
import java.io.InputStreamReader;
import java.io.OutputStream;
import java.io.OutputStreamWriter;
import java.nio.charset.Charset;
import java.nio.charset.StandardCharsets;

class Codec {
    void m(String text, byte[] bytes, InputStream in, OutputStream out, String enc, Unknown u) throws Exception {
        Charset a = Charset.forName("UTF-8");                       // expect: standard-charset-lookup
        Charset b = Charset.forName("iso-8859-1");                  // expect: standard-charset-lookup
        byte[] c = text.getBytes("UTF-8");                          // expect: standard-charset-lookup
        String d = new String(bytes, "US-ASCII");                   // expect: standard-charset-lookup
        String e = new String(bytes, 0, 4, "UTF-16LE");             // expect: standard-charset-lookup
        InputStreamReader f = new InputStreamReader(in, "UTF8");    // expect: standard-charset-lookup
        OutputStreamWriter g = new OutputStreamWriter(out, "utf-8"); // expect: standard-charset-lookup

        Charset ok1 = Charset.forName("windows-1252");              // not a standard charset
        Charset ok2 = Charset.forName(enc);
        byte[] ok3 = text.getBytes(StandardCharsets.UTF_8);
        byte[] ok4 = u.getBytes("UTF-8");                           // unknown receiver type
        String ok5 = new String(bytes, StandardCharsets.UTF_8);
        Object ok6 = Class.forName("UTF-8");                        // not Charset.forName
        String ok7 = new String("UTF-8");
    }
}
