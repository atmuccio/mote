import javax.inject.Inject;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.beans.factory.annotation.Value;

class OrderService {
    @Autowired                                                       // expect: field-injection
    private OrderRepository repository;

    @Inject                                                          // expect: field-injection
    Mailer mailer;

    @org.springframework.beans.factory.annotation.Autowired          // expect: field-injection
    private Clock clock;

    @Value("${timeout}")
    private int timeout;

    private final Audit audit;

    @Autowired
    OrderService(Audit audit) {
        this.audit = audit;
    }

    @Autowired
    void setMailer(Mailer mailer) {
        this.mailer = mailer;
    }

    interface OrderRepository {}
    interface Mailer {}
    interface Clock {}
    interface Audit {}
}
