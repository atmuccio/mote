// mote-test: test-file
class WorkerTest {
    void waitABit() {
        try {
            Thread.sleep(100);
        } catch (InterruptedException e) {
            e.printStackTrace();
        }
    }
}
