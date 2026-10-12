import java.util.concurrent.ExecutorService;
import java.util.concurrent.ThreadPoolExecutor;

// No known Java release: an edge the JDK added in Java 19 proves nothing.
abstract class CheckedPool extends ThreadPoolExecutor implements AutoCloseable {
    CheckedPool() { super(1, 1, 0, null, null); }
}

interface Service extends ExecutorService, AutoCloseable { }
