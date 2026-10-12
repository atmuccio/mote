import java.util.List;

class OverrideDropsSynchronized {
    static class Counter {
        protected int count;
        public synchronized void increment() { count++; }
        public synchronized void add(int n) { count += n; }
        public synchronized int get() { return count; }
        public synchronized void reset() { count = 0; }
        public synchronized void clear() { count = 0; }
        public synchronized void log(String s) { }
        public synchronized void set(List<String> v) { }
        public void plain() { }
        private synchronized void secret() { }
        public static synchronized void util() { }
    }

    static class LoggingCounter extends Counter {
        @Override
        public void increment() {                                    // expect: override-drops-synchronized
            System.out.println("inc");
            count++;
        }

        @Override
        public void set(List<String> v) {                            // expect: override-drops-synchronized
            count = v.size();
        }

        // Compliant
        @Override
        public synchronized void add(int n) { count += 2 * n; }

        @Override
        public int get() { return super.get(); }                     // delegates to the locked method

        @Override
        public void reset() { throw new UnsupportedOperationException(); }

        @Override
        public void clear() {
            synchronized (this) {
                count = -1;
            }
        }

        @Override
        public void log(String s) { }

        public void add(long n) { }                                  // an overload, not an override
        public void plain() { count = 1; }
        public void secret() { count = 2; }
        public static void util() { }
    }

    static class Deeper extends LoggingCounter {
        @Override
        public void reset() { count = 5; }                           // LoggingCounter.reset is not synchronized
    }

    Counter anonymous() {
        return new Counter() {
            @Override
            public void add(int n) {                                  // expect: override-drops-synchronized
                count -= n;
            }
        };
    }

    static class External extends java.util.Vector<String> {
        @Override
        public int size() { return 0; }                              // superclass from another file
    }
}
