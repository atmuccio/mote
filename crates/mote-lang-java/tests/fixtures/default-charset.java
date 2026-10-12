// mote-test: params java_release=11
import java.io.BufferedReader;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.net.URL;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import javax.crypto.Mac;

class Hashing {
    byte[] positives(String password, MessageDigest sha, Mac mac, URL url) throws Exception {
        sha.update(password.getBytes());                                    // expect: default-charset
        byte[] h = sha.digest(password.getBytes());                         // expect: default-charset
        byte[] m = mac.doFinal("clé".getBytes());                           // expect: default-charset
        BufferedReader a = new BufferedReader(new InputStreamReader(getClass().getResourceAsStream("/t.txt")));  // expect: default-charset
        InputStreamReader b = new InputStreamReader(url.openStream());      // expect: default-charset
        return h;
    }

    void negatives(String password, MessageDigest sha, InputStream in, Process p, Other o) throws Exception {
        sha.update(password.getBytes(StandardCharsets.UTF_8));
        sha.update("abc".getBytes());                                       // ASCII literal
        byte[] raw = password.getBytes();                                   // not hashed here
        String s = new String(raw);
        InputStreamReader c = new InputStreamReader(in);                    // origin unknown
        InputStreamReader d = new InputStreamReader(p.getInputStream());
        InputStreamReader e = new InputStreamReader(getClass().getResourceAsStream("/t.txt"), StandardCharsets.UTF_8);
        o.update(password.getBytes());                                      // not a hashing class
    }

    interface Other { void update(byte[] b); }
}
