class Resource {
    protected void finalize(boolean force) throws Throwable {       // expect: object-method-name-typo
        release();
        super.finalize();
    }

    void release() {}
}

class Cleaner {
    public void finalize(Object ctx) {                              // expect: object-method-name-typo
        close();
        super.finalize();
    }

    void close() {}

    protected void finalize() throws Throwable {}                   // the real finalizer
}

interface TaskContainer {
    void finalize(Runnable task);                                   // a domain verb
}

class Workflow {
    public void finalize(Runnable task) {                           // a domain verb
        task.run();
    }

    static void finalize(int x) throws Throwable {}
}
