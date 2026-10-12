import java.util.logging.Logger;

class PaymentHandler {
    private static final Logger LOGGER = Logger.getLogger(PaymentService.class.getName());   // expect: logger-for-wrong-class
    private static final Logger OWN = Logger.getLogger(PaymentHandler.class.getName());
}
