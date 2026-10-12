class Memory {
    void release(java.util.Map<String, Object> cache) {
        cache.clear();
        System.gc();                                               // expect: explicit-gc
        Runtime.getRuntime().gc();                                 // expect: explicit-gc
        Runtime rt = Runtime.getRuntime();
        rt.gc();                                                   // expect: explicit-gc
        java.lang.System.gc();                                     // expect: explicit-gc
    }

    // Unrelated gc() methods.
    void gc() { }
    void other(Collector c) {
        gc();
        c.gc();
        this.gc();
    }
}
