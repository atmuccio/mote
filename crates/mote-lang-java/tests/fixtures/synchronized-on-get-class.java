class Registry {
    private static int instances;

    void wrong() {
        synchronized (getClass()) {                     // expect: synchronized-on-get-class
            instances++;
        }
        synchronized (this.getClass()) {                // expect: synchronized-on-get-class
            instances--;
        }
    }

    void fine(Object other) {
        synchronized (Registry.class) {
            instances++;
        }
        synchronized (other.getClass()) {
            instances++;
        }
        Runnable r = new Runnable() {
            @Override
            public void run() {
                synchronized (getClass()) {
                    instances++;
                }
            }
        };
    }
}

final class Sealed {
    private static int count;

    void fine() {
        synchronized (getClass()) {
            count++;
        }
    }
}

enum Mode {
    A;

    void fine() {
        synchronized (getClass()) {
        }
    }
}

record Point(int x) {
    void fine() {
        synchronized (getClass()) {
        }
    }
}
