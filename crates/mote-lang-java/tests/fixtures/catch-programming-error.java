import java.util.concurrent.locks.ReentrantLock;

class Addresses {
    String city(Customer customer) {
        try {
            return customer.getAddress().getCity();
            // expect-next-line: catch-programming-error
        } catch (NullPointerException e) {
            return "";
        }
    }

    void release(ReentrantLock lock) {
        try {
            lock.unlock();
            // expect-next-line: catch-programming-error
        } catch (IllegalMonitorStateException ignored) {
            lock.lock();
        }
    }

    Object multi(Object o) {
        try {
            return ((String) o).trim();
            // expect-next-line: catch-programming-error
        } catch (ClassCastException | java.lang.NullPointerException e) {
            return null;
        }
    }

    void negatives(Customer customer) {
        try {
            customer.refresh();
        } catch (IllegalStateException e) {
            customer.reset();
        }
        try {
            customer.refresh();
        } catch (NullPointerException expected) {
            customer.reset();
        }
        try {
            customer.refresh();
        } catch (NullPointerException e) {
            // the driver throws this when the statement has no parameters
            customer.reset();
        }
        if (customer == null) {
            throw new NullPointerException("customer");
        }
        // Kept out of the generic handler below.
        try {
            customer.write(null);
        } catch (NullPointerException npe) {
            throw npe;
        } catch (Exception e) {
            customer.reset();
        }
        // Rethrown unless it is a known library failure.
        try {
            customer.parse();
        } catch (NullPointerException e) {
            if (!String.valueOf(e.getMessage()).contains("known issue")) {
                throw e;
            }
        }
    }
}

// Wrapping the caught exception as a cause does not count as throwing it on.
class Wrapping {
    String city(Customer customer) {
        try {
            return customer.getAddress().getCity();
            // expect-next-line: catch-programming-error
        } catch (NullPointerException e) {
            throw new IllegalStateException("no city", e);
        }
    }
}
