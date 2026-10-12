import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.ScheduledThreadPoolExecutor;
import java.util.concurrent.ThreadPoolExecutor;

class Timers {
    ScheduledExecutorService wrong(ScheduledThreadPoolExecutor stpe) {
        ScheduledExecutorService a = Executors.newScheduledThreadPool(0);          // expect: scheduled-pool-without-core-threads
        ScheduledThreadPoolExecutor b = new ScheduledThreadPoolExecutor(0);        // expect: scheduled-pool-without-core-threads
        stpe.setCorePoolSize(0);                                                   // expect: scheduled-pool-without-core-threads
        return a;
    }

    void fine(ThreadPoolExecutor tpe, Pool custom, int n) {
        ScheduledExecutorService a = Executors.newScheduledThreadPool(1);
        ScheduledExecutorService b = Executors.newSingleThreadScheduledExecutor();
        ScheduledExecutorService c = Executors.newScheduledThreadPool(n);
        tpe.setCorePoolSize(0);
        custom.newScheduledThreadPool(0);
        ScheduledThreadPoolExecutor d = new ScheduledThreadPoolExecutor(2);
    }

    interface Pool { void newScheduledThreadPool(int n); }
}
