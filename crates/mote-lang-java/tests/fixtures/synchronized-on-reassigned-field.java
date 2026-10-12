import java.util.ArrayList;
import java.util.List;

class Listeners {
    private List<Runnable> listeners = new ArrayList<>();
    private List<Runnable> others = new ArrayList<>();
    private final Object lock = new Object();

    void wrong(Runnable l) {
        synchronized (listeners) {                              // expect: synchronized-on-reassigned-field
            listeners = new ArrayList<>(listeners);
            listeners.add(l);
        }
        synchronized (this.others) {                            // expect: synchronized-on-reassigned-field
            if (others.size() > 10) {
                this.others = new ArrayList<>();
            }
        }
    }

    void fine(Runnable l, List<Runnable> param) {
        synchronized (lock) {
            listeners = new ArrayList<>(listeners);
        }
        synchronized (listeners) {
            listeners.add(l);
        }
        synchronized (others) {
            List<Runnable> others = new ArrayList<>();
            others = new ArrayList<>(param);
        }
        synchronized (param) {
            param = new ArrayList<>();
        }
        synchronized (listeners) {
            Runnable r = () -> { listeners = new ArrayList<>(); };
        }
        synchronized (listeners) {
            listeners = null;
        }
    }
}
