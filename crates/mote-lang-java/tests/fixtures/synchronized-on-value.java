class Locks {
    private static final String LOCK = "LOCK";
    private final Integer counter = 0;
    private Boolean flag = Boolean.FALSE;
    private final Object monitor = new Object();
    private final String unique = new String("lock");
    private final Long boxedNew = new Long(1L);

    void positives(String key) {
        synchronized ("lock") { work(); }                     // expect: synchronized-on-value
        synchronized (LOCK) { work(); }                       // expect: synchronized-on-value
        synchronized (counter) { work(); }                    // expect: synchronized-on-value
        synchronized (this.flag) { work(); }                  // expect: synchronized-on-value
        synchronized (Boolean.TRUE) { work(); }               // expect: synchronized-on-value
        synchronized (key) { work(); }                        // expect: synchronized-on-value
        String local = key.trim();
        synchronized ((local)) { work(); }                    // expect: synchronized-on-value
        Runnable r = () -> {
            synchronized (LOCK) { work(); }                   // expect: synchronized-on-value
        };
    }

    void negatives(Object o, Unknown u) {
        synchronized (this) { work(); }
        synchronized (monitor) { work(); }
        synchronized (Locks.class) { work(); }
        synchronized (unique) { work(); }                     // unique instance
        synchronized (this.unique) { work(); }
        synchronized (boxedNew) { work(); }
        synchronized (key().intern()) { work(); }             // method result
        synchronized (o) { work(); }
        synchronized (u) { work(); }
        synchronized (u.name) { work(); }                     // unknown type
    }

    String key() { return "k"; }
}
