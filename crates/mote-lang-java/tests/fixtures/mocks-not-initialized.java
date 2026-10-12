// mote-test: test-file
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;
import org.mockito.InjectMocks;
import org.mockito.Mock;

@DisplayName("orders")
class OrderServiceTest {
    @Mock                                                            // expect: mocks-not-initialized
    OrderRepository repository;

    @InjectMocks
    OrderService service;

    @Test
    void saves() {
        service.save();
    }

    interface OrderRepository {}

    static class OrderService {
        void save() {}
    }
}
