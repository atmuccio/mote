import java.util.List;
import java.util.concurrent.Executors;
import java.util.concurrent.ThreadFactory;

class Threads {
    private Thread worker;

    void positives(Runnable task, ThreadFactory factory) {
        Thread t = new Thread(task);
        t.run();                                // expect: thread-run-direct
        worker.run();                           // expect: thread-run-direct
        this.worker.run();                      // expect: thread-run-direct
        new Thread(task).run();                 // expect: thread-run-direct
        factory.newThread(task).run();          // expect: thread-run-direct
        MyThread mine = new MyThread();
        mine.run();                             // expect: thread-run-direct
        Thread.currentThread().run();           // expect: thread-run-direct
    }

    void negatives(Runnable task, List<Thread> threads, Unknown u) {
        task.run();                             // a Runnable, not a Thread
        worker.start();
        u.thread().run();                       // unknown type
        threads.forEach(th -> th.run());        // lambda parameter type is unknown
        Runnable r = () -> {};
        r.run();
    }

    static class MyThread extends Thread {
        @Override
        public void run() {
            super.run();
            helper();
        }

        void again() {
            run();                              // unqualified call inside the thread class
        }

        void helper() {}
    }
}
