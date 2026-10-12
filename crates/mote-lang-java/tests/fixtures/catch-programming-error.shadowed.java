class Shadowed {
    static class NullPointerException extends RuntimeException {}

    void refresh(Customer customer) {
        try {
            customer.refresh();
        } catch (NullPointerException e) {
            customer.reset();
        }
    }
}
