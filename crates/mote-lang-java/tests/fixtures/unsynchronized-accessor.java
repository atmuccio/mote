class Settings {
    private int limit;
    private boolean enabled;
    private long stamp;
    private static String mode;

    public synchronized void setLimit(int limit) {
        this.limit = limit;
    }

    public int getLimit() {                                        // expect: unsynchronized-accessor
        return limit;
    }

    public synchronized boolean isEnabled() {
        return enabled;
    }

    public void setEnabled(boolean enabled) {                      // expect: unsynchronized-accessor
        this.enabled = enabled;
    }

    public static synchronized void setMode(String m) {
        mode = m;
    }

    public static String getMode() {                               // expect: unsynchronized-accessor
        return mode;
    }

    public synchronized void setStamp(long stamp) {
        this.stamp = stamp;
    }

    public synchronized long getStamp() {
        return stamp;
    }
}

class Compliant {
    private volatile int size;
    private int count;
    private int other;
    private final Object lock = new Object();
    private int guarded;

    public synchronized void setSize(int size) {
        this.size = size;
    }

    public int getSize() {
        return size;
    }

    public void setCount(int count) {
        this.count = count;
    }

    public int getCount() {
        return count;
    }

    public synchronized void setOther(int value) {
        this.other = value * 2;
    }

    public int getOther() {
        return other;
    }

    public void setGuarded(int g) {
        synchronized (lock) {
            guarded = g;
        }
    }

    public synchronized int getGuarded() {
        return guarded;
    }

    public synchronized void setInherited(int v) {
        inherited = v;
    }

    public int getInherited() {
        return inherited;
    }
}

@lombok.Getter
class WithLombok {
    private int level;

    public synchronized void setLevel(int level) {
        this.level = level;
    }

    public int getLevel() {
        return level;
    }
}
