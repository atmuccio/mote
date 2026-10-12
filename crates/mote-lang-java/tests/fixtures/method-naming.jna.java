import com.sun.jna.Structure;
import com.sun.jna.platform.win32.Advapi32;

// JNA bindings mirror native symbol names: not reported.
public interface Advapi32Ex extends Advapi32 {
    boolean ChangeServiceConfig2(Object service, int infoLevel);
    boolean DeleteService(Object serviceHandle);

    class SERVICE_INFO extends Structure {
        public int dwServiceType;
        public int Reserved_Field;
    }
}
