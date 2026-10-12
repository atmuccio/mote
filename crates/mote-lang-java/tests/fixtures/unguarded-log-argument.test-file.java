// mote-test: test-file
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

class OrderServiceTest {
    private static final Logger log = LoggerFactory.getLogger(OrderServiceTest.class);

    void logs(Object order) {
        log.debug("Order {}", order.toString());                   // test code: not reported
    }
}
