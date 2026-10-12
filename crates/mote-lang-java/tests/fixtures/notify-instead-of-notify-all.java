import java.util.ArrayDeque;
import java.util.Queue;

class Buffer {
    private final Queue<String> items = new ArrayDeque<>();
    private final Object lock = new Object();

    synchronized void put(String s) {
        items.add(s);
        notify();                                                   // expect: notify-instead-of-notify-all
    }

    void signal() {
        synchronized (lock) {
            lock.notify();                                          // expect: notify-instead-of-notify-all
        }
        Runnable r = () -> {
            synchronized (this) { this.notify(); }                  // expect: notify-instead-of-notify-all
        };
    }

    // Compliant
    synchronized void putAll(String s) {
        items.add(s);
        notifyAll();
    }

    void other(Listener listener, Event e) {
        listener.notify(e);                                         // someone's own notify(Event)
        notify(e);
    }

    void notify(Event e) { }

    interface Listener { void notify(Event e); }

    static class Event { }
}
