// mote-test: params java_release=8
import java.io.InputStream;
import java.util.concurrent.ScheduledThreadPoolExecutor;

// Before Java 19 an ExecutorService is not AutoCloseable: listing it is required.
abstract class CheckedPool extends ScheduledThreadPoolExecutor implements AutoCloseable {
    CheckedPool() { super(1); }
}

// InputStream is Closeable (hence AutoCloseable) since Java 7: still redundant.
abstract class Source extends InputStream implements AutoCloseable { }                  // expect: redundant-inheritance-declaration
