class Shipping {
    private static final boolean DEBUG = false;

    void process(Order order, boolean ready) {
        if (false) {                                                // expect: constant-if-condition
            audit(order);
        }
        if (true) {                                                 // expect: constant-if-condition
            ship(order);
        } else {
            hold(order);
        }
        if ((true)) {                                               // expect: constant-if-condition
            ship(order);
        }
        if (ready) {
            ship(order);
        } else if (false) {                                         // expect: constant-if-condition
            hold(order);
        }
        Runnable r = () -> {
            if (true) return;                                       // expect: constant-if-condition
        };

        // Not reported.
        if (DEBUG) {
            audit(order);
        }
        if (DEBUG && ready) {
            audit(order);
        }
        if (ready == true) {
            ship(order);
        }
        if (!ready) {
            hold(order);
        }
        while (true) {
            if (ready) {
                break;
            }
        }
        do {
            ship(order);
        } while (false);
        boolean b = true ? ready : !ready;
    }

    void audit(Order o) {
    }

    void ship(Order o) {
    }

    void hold(Order o) {
    }

    record Order(String id) {
    }
}
