// mote-test: params java_release=21
import java.security.MessageDigest;

class Modern {
    byte[] hash(MessageDigest md, String s) {
        return md.digest(s.getBytes());                                     // UTF-8 by default since Java 18
    }
}
