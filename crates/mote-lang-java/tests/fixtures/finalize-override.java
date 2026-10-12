class Finalizers {
    @Override
    protected void finalize() throws Throwable {                    // expect: finalize-override
        try { close(); } finally { super.finalize(); }
    }

    static class Plain {
        public void finalize() { }                                  // expect: finalize-override
    }

    Object anon = new Object() {
        @Override protected void finalize() { release(); }          // expect: finalize-override
    };

    // negatives
    static class Guarded {
        protected final void finalize() { }                         // finalizer-attack guard
    }

    static class GuardedDelegating {
        @Override
        protected final void finalize() throws Throwable {
            // finalizer attack remediation
            super.finalize();
        }
    }

    static class FinalButWorking {
        @Override
        protected final void finalize() throws Throwable {          // expect: finalize-override
            release();
            super.finalize();
        }

        void release() { }
    }

    static class NotFinalDelegating {
        @Override
        protected void finalize() throws Throwable {                // expect: finalize-override
            super.finalize();
        }
    }

    void finalize(int reason) { }                                   // an overload, not the finalizer

    void finalizeOrder() { }

    void caller() {
        finalize(3);
    }
}
