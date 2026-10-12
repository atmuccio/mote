import java.util.List;

class Reports {
    String describeOrder(Order order) {
        StringBuilder sb = new StringBuilder(order.id());
        sb.append(": ").append(order.total());
        return sb.toString();
    }

    String describeInvoice(Order order) {                                   // expect: identical-methods
        StringBuilder sb = new StringBuilder(order.id());
        // same thing, different comment
        sb.append(": ")
          .append(order.total());
        return sb.toString();
    }

    String describeCredit(Order order) {                                    // expect: identical-methods
        StringBuilder sb = new StringBuilder(order.id());
        sb.append(": ").append(order.total());
        return sb.toString();
    }

    // Different parameter types: overloads that cannot share the body.
    int sum(int[] values) {
        int total = 0;
        for (int i = 0; i < values.length; i++) total += values[i];
        return total;
    }

    int sum(long[] values) {
        int total = 0;
        for (int i = 0; i < values.length; i++) total += values[i];
        return total;
    }

    // Different parameter names: the bodies differ.
    String left(Order a) {
        StringBuilder sb = new StringBuilder(a.id());
        sb.append(": ").append(a.total());
        return sb.toString();
    }

    // Too short.
    int one() {
        return 1;
    }

    int uno() {
        return 1;
    }

    // Similar but not identical.
    String describeRefund(Order order) {
        StringBuilder sb = new StringBuilder(order.id());
        sb.append(" - ").append(order.total());
        return sb.toString();
    }

    Runnable callbacks(List<String> names) {
        new Thread(new Runnable() {
            @Override
            public void run() {
                names.clear();
                names.add("a");
                names.add("b");
            }
        }).start();
        return new Runnable() {
            @Override
            public void run() {
                names.clear();
                names.add("a");
                names.add("b");
            }
        };
    }

    static class Nested {
        String describeOrder(Order order) {                                 // another class
            StringBuilder sb = new StringBuilder(order.id());
            sb.append(": ").append(order.total());
            return sb.toString();
        }
    }
}

enum Mode {
    FAST {
        @Override
        void apply(List<String> steps) {
            steps.clear();
            steps.add("x");
            steps.add("y");
        }
    },
    SLOW {
        @Override
        void apply(List<String> steps) {
            steps.clear();
            steps.add("x");
            steps.add("y");
        }
    };

    abstract void apply(List<String> steps);
}

interface Counts {
    int known();
    int active();
}

class NoCluster implements Counts {
    private boolean initialized;

    @Override
    public int known() {
        if (!initialized) {
            throw new IllegalStateException("not initialized");
        }
        return 1;
    }

    @Override
    public int active() {
        if (!initialized) {
            throw new IllegalStateException("not initialized");
        }
        return 1;
    }

    public int configured() {                                               // expect: identical-methods
        if (!initialized) {
            throw new IllegalStateException("not initialized");
        }
        return 1;
    }
}
