class ExplicitFinalizeCall {
    static class Handle {
        @Override
        protected void finalize() throws Throwable {
            try {
                release();
            } finally {
                super.finalize();
            }
        }

        void release() { }

        void dispose() throws Throwable {
            finalize();                                             // expect: explicit-finalize-call
        }
    }

    void close(Handle h) throws Throwable {
        h.finalize();                                               // expect: explicit-finalize-call
        this.finalize();                                            // expect: explicit-finalize-call
        System.runFinalizersOnExit(true);                           // expect: explicit-finalize-call
        Runtime.runFinalizersOnExit(true);                          // expect: explicit-finalize-call
    }

    // Compliant
    void fine(Builder b, Digest d) {
        b.finalize("done");                                         // takes an argument: not Object.finalize
        d.finish();
        System.runFinalization();
    }

    interface Builder { void finalize(String s); }
    interface Digest { void finish(); }
}
