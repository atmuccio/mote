import java.util.concurrent.ScheduledThreadPoolExecutor;
import java.util.concurrent.TimeUnit;

class EvictionTimers {
    private static ScheduledThreadPoolExecutor executor;
    private ScheduledThreadPoolExecutor timer;
    private ScheduledThreadPoolExecutor backup;

    // Teardown: the pool is shut down first, then its core threads are released.
    static synchronized void cancel(boolean idle) throws InterruptedException {
        if (idle && executor != null) {
            executor.shutdown();
            executor.setCorePoolSize(0);
            executor = null;
        }
        if (executor != null) {
            executor.shutdownNow();
            executor.awaitTermination(1, TimeUnit.SECONDS);
            executor.setCorePoolSize(0);
        }
    }

    void close() {
        this.timer.shutdown();
        timer.setCorePoolSize(0);
    }

    // Another pool was shut down, not this one.
    void shrink(Holder holder) {
        holder.timer.shutdown();
        backup.shutdown();
        timer.setCorePoolSize(0);                                      // expect: scheduled-pool-without-core-threads
    }

    // Shut down only after the core size change.
    void later() {
        timer.setCorePoolSize(0);                                      // expect: scheduled-pool-without-core-threads
        timer.shutdown();
    }

    static class Holder {
        ScheduledThreadPoolExecutor timer;
    }
}
