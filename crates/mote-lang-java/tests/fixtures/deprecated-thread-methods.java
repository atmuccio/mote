import java.util.List;

class Stopper {
    private Thread worker;

    void positives(Runnable task) {
        Thread t = new Thread(task);
        t.stop();                                   // expect: deprecated-thread-methods
        worker.suspend();                           // expect: deprecated-thread-methods
        this.worker.resume();                       // expect: deprecated-thread-methods
        Thread.currentThread().stop();              // expect: deprecated-thread-methods
        new Worker().stop();                        // expect: deprecated-thread-methods
    }

    void negatives(Service service, List<Thread> threads, Unknown u, Timer timer) {
        service.stop();                             // not a thread
        worker.interrupt();
        worker.stop(1);                             // different arity
        u.thread().stop();                          // unknown type
        threads.forEach(th -> th.stop());           // lambda parameter type is unknown
        timer.stop();
    }

    static class Worker extends Thread {
        void shutdown() {
            stop();                                 // expect: deprecated-thread-methods
        }
    }

    static class Service {
        void stop() {}
    }

    static class Timer {
        void stop() {}

        void again() {
            stop();                                 // Timer's own stop()
        }
    }
}
