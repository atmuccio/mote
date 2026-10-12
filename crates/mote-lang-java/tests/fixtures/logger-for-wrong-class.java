import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

class InvoiceService extends AbstractService implements Billing {
    private static final Logger log = LoggerFactory.getLogger(OrderService.class);           // expect: logger-for-wrong-class
    private static final Logger ok = LoggerFactory.getLogger(InvoiceService.class);
    private static final Logger base = LoggerFactory.getLogger(AbstractService.class);       // supertype: accepted
    private static final Logger iface = LoggerFactory.getLogger(Billing.class);              // supertype: accepted
    private final Logger dynamic = LoggerFactory.getLogger(getClass());
    private static final Logger named = LoggerFactory.getLogger("billing.audit");

    static class Helper {
        private static final Logger outer = LoggerFactory.getLogger(InvoiceService.class);   // enclosing class: accepted
        private static final Logger wrong = LoggerFactory.getLogger(Other.class);            // expect: logger-for-wrong-class
    }

    void method() {
        Logger local = LoggerFactory.getLogger(OrderService.class);                         // not a field
    }

    Runnable r = new Runnable() {
        private final Logger anon = LoggerFactory.getLogger(InvoiceService.class);           // anonymous: enclosing class
        public void run() {}
    };
}

class Registry {
    private static final Logger log = Loggers.getLogger(OrderService.class);                 // unknown factory
}

class Assertion {
    // Deliberately the element's category: the GUI toggles debug output for it.
    private static final Logger log = LoggerFactory.getLogger(ElementGui.class);
    private static final Logger trailing = LoggerFactory.getLogger(ElementGui.class); // expect: logger-for-wrong-class
}

interface ContentLoader {
    class Validator {
        private static final Logger LOG = LoggerFactory.getLogger(DefaultContentLoader.class); // related class
        private static final Logger LOG2 = LoggerFactory.getLogger(ContentLoaderImpl.class);
    }
}
