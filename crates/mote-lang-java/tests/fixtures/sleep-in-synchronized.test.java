// mote-test: test-file
class ContentionTest {
    private final Object lock = new Object();

    void holdsLockOnPurpose() throws InterruptedException {
        synchronized (lock) {
            Thread.sleep(200);
        }
    }
}
