// mote-test: test-file
class ConnectionManagerTest {
    void reserves() {
        reserve("node1");
        reserve("node1");
        reserve("node1");
    }
    void reserve(String id) {}
}
