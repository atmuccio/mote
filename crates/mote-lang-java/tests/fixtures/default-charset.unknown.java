import java.security.MessageDigest;

class UnknownRelease {
    byte[] hash(MessageDigest md, String s) {
        return md.digest(s.getBytes());                                     // release unknown: silent
    }
}
