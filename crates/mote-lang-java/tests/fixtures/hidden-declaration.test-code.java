// mote-test: test-file
class OrderTest {
    private Order<String> tested;

    void createsItsOwn() {
        Order<String> tested = new Order<>("t");
        tested.add(1, 2);
    }
}
