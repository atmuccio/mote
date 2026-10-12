import java.util.ArrayDeque;
import java.util.Queue;
import java.util.concurrent.TimeUnit;

class Poller {
    private final Queue<String> queue = new ArrayDeque<>();

    String take() throws InterruptedException {
        synchronized (queue) {
            while (queue.isEmpty()) {
                Thread.sleep(100);                                  // expect: sleep-in-synchronized
            }
            return queue.poll();
        }
    }

    synchronized void retry() throws InterruptedException {
        for (int i = 0; i < 3; i++) {
            TimeUnit.SECONDS.sleep(1);                              // expect: sleep-in-synchronized
        }
    }

    synchronized void pause() throws InterruptedException {
        Thread.currentThread().sleep(10);                           // expect: sleep-in-synchronized
    }

    String fine() throws InterruptedException {
        synchronized (queue) {
            while (queue.isEmpty()) {
                queue.wait(100);
            }
        }
        Thread.sleep(100);
        synchronized (queue) {
            Runnable later = () -> {
                try {
                    Thread.sleep(5);
                } catch (InterruptedException e) {
                    Thread.currentThread().interrupt();
                }
            };
            new Thread(later).start();
            Object o = new Object() {
                void nap() throws InterruptedException { Thread.sleep(1); }
            };
        }
        return queue.poll();
    }

    synchronized void customSleep(Sleeper s) {
        s.sleep(10);
        sleep(10);
    }

    void sleep(int ms) { }

    interface Sleeper { void sleep(int ms); }
}
