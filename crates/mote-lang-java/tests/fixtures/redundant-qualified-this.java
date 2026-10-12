class Counter {
    private int count;

    void increment() {
        Counter.this.count++;                                       // expect: redundant-qualified-this
        Runnable r = () -> Counter.this.count++;                    // expect: redundant-qualified-this
        register(Counter.this);                                     // expect: redundant-qualified-this
        this.count++;
    }

    void anonymous() {
        Runnable r = new Runnable() {
            public void run() {
                Counter.this.count++;                               // the enclosing instance: needed
            }
        };
    }

    class Inner {
        int count;

        void m() {
            Counter.this.count++;                                   // needed
            Inner.this.count++;                                     // expect: redundant-qualified-this
        }
    }

    void local() {
        class Local {
            void m() {
                Counter.this.count++;                               // needed
            }
        }
    }

    void register(Object o) { }
}

enum Mode {
    ON;

    Mode self() {
        return Mode.this;                                           // expect: redundant-qualified-this
    }
}
