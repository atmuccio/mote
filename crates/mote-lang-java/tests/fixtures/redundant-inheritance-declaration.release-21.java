// mote-test: params java_release=21
import java.io.InputStream;
import java.util.concurrent.ScheduledThreadPoolExecutor;
import java.util.concurrent.ThreadPoolExecutor;

// From Java 19 on, every ExecutorService is AutoCloseable.
abstract class CheckedPool extends ScheduledThreadPoolExecutor implements AutoCloseable { // expect: redundant-inheritance-declaration
    CheckedPool() { super(1); }
}

abstract class Source extends InputStream implements AutoCloseable { }                  // expect: redundant-inheritance-declaration
