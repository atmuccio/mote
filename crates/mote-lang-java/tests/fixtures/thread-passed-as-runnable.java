import java.util.concurrent.CompletableFuture;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;

class Workers {
    ExecutorService pool = Executors.newFixedThreadPool(2);
    ScheduledExecutorService timer = Executors.newScheduledThreadPool(1);

    void start(Runnable task) {
        Thread worker = new Thread(task, "worker");
        worker.setDaemon(true);
        pool.execute(worker);                                      // expect: thread-passed-as-runnable
        pool.submit(worker);                                       // expect: thread-passed-as-runnable
        timer.schedule(worker, 1, TimeUnit.SECONDS);               // expect: thread-passed-as-runnable
        CompletableFuture.runAsync(worker);                        // expect: thread-passed-as-runnable
        new Thread(worker).start();                                // expect: thread-passed-as-runnable
        new Thread(new Thread(task), "outer").start();             // expect: thread-passed-as-runnable
        new Thread(null, new Poller(), "p").start();               // expect: thread-passed-as-runnable

        // Fine: runnables and lambdas.
        pool.execute(task);
        pool.execute(() -> task.run());
        pool.submit(this::poll);
        new Thread(task).start();
        new Thread(new Job()).start();
        worker.start();
    }

    void poll() { }

    // Unknown executor type: silent.
    void other(MyExecutor ex, Thread t) {
        ex.execute(t);
    }

    static class Poller extends Thread {
        @Override public void run() { }
    }

    static class Job implements Runnable {
        @Override public void run() { }
    }
}

class Self extends Thread {
    void again(ExecutorService pool) {
        pool.execute(this);                                        // expect: thread-passed-as-runnable
    }
}
