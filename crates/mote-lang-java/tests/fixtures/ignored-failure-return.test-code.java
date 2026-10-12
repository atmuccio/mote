// mote-test: test-file
import java.io.File;
import java.util.concurrent.locks.Lock;

class StorageTest {
    void cleanup(File tmp, Lock lock) {
        tmp.delete();                                               // best-effort cleanup in tests
        lock.tryLock();                                             // expect: ignored-failure-return
    }
}
