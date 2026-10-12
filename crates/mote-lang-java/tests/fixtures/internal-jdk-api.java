import sun.misc.BASE64Encoder;                                     // expect: internal-jdk-api
import sun.misc.Unsafe;                                            // expect: internal-jdk-api
import jdk.internal.misc.VM;                                       // expect: internal-jdk-api
import com.sun.org.apache.xerces.internal.jaxp.DocumentBuilderFactoryImpl; // expect: internal-jdk-api
import sun.security.x509.*;                                        // expect: internal-jdk-api
import static sun.misc.Unsafe.ARRAY_BYTE_BASE_OFFSET;              // expect: internal-jdk-api

// Supported com.sun APIs and third-party com.sun packages: fine.
import com.sun.net.httpserver.HttpServer;
import com.sun.management.OperatingSystemMXBean;
import com.sun.messaging.jmq.io.Packet;
import java.util.Base64;

class Codec {
    sun.nio.ch.DirectBuffer direct;                                // expect: internal-jdk-api
    Object sun = null;

    String encode(byte[] bytes) {
        Object o = sun;
        String name = "sun.misc.Unsafe";                           // strings: fine
        java.util.List<sun.reflect.ReflectionFactory> l = null;    // expect: internal-jdk-api
        return Base64.getEncoder().encodeToString(bytes);
    }
}
