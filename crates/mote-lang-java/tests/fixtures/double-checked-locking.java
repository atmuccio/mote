class Lazy {
    private Service instance;
    private static Registry registry;
    private volatile Service safe;
    private String name;
    private Service viaThis;
    private final Object lock = new Object();

    Service get() {
        if (instance == null) {
            synchronized (this) {                                   // expect: double-checked-locking
                if (instance == null) {
                    instance = new Service();
                }
            }
        }
        return instance;
    }

    static Registry registry() {
        if (registry == null) {
            synchronized (Lazy.class) {                             // expect: double-checked-locking
                if (null == registry)
                    registry = load();
            }
        }
        return registry;
    }

    Service viaThis() {
        if (this.viaThis == null) {
            synchronized (lock) {                                   // expect: double-checked-locking
                log("init");
                if (viaThis == null) {
                    Service s = new Service();
                    s.start();
                    this.viaThis = s;
                }
            }
        }
        return viaThis;
    }

    Service nested() {
        if ((instance == null))
            synchronized (lock) {                                   // expect: double-checked-locking
                if (instance == null) { instance = new Service(); }
            }
        return instance;
    }

    // negatives
    Service volatileField() {
        if (safe == null) {
            synchronized (this) {
                if (safe == null) {
                    safe = new Service();
                }
            }
        }
        return safe;
    }

    String immutable() {
        if (name == null) {
            synchronized (this) {
                if (name == null) {
                    name = compute();
                }
            }
        }
        return name;
    }

    Service localIdiom() {
        Service result = safe;
        if (result == null) {
            synchronized (this) {
                result = safe;
                if (result == null) {
                    safe = result = new Service();
                }
            }
        }
        return result;
    }

    Service singleCheck() {
        synchronized (this) {
            if (instance == null) {
                instance = new Service();
            }
        }
        return instance;
    }

    Service differentField() {
        if (instance == null) {
            synchronized (this) {
                if (viaThis == null) {
                    instance = new Service();
                }
            }
        }
        return instance;
    }

    Service elseBranch() {
        if (instance == null) {
            log("x");
        } else {
            synchronized (this) {
                if (instance == null) {
                    instance = new Service();
                }
            }
        }
        return instance;
    }

    Service noAssign() {
        if (instance == null) {
            synchronized (this) {
                if (instance == null) {
                    log("missing");
                }
            }
        }
        return instance;
    }

    Service shadowedByLocal() {
        Service instance = null;
        if (instance == null) {
            synchronized (this) {
                if (instance == null) {
                    instance = new Service();
                }
            }
        }
        return instance;
    }
}

class Inherited extends Unknown {
    Service get() {
        if (cached == null) {                    // field of an unknown superclass
            synchronized (this) {
                if (cached == null) {
                    cached = new Service();
                }
            }
        }
        return cached;
    }
}
