import com.sun.jna.Structure;

// JNA maps public Structure fields onto native memory: not reported.
public class Shell32Ex {
    public static class SHELLEXECUTEINFO extends Structure {
        public int cbSize;
        public String lpVerb;
    }
}
