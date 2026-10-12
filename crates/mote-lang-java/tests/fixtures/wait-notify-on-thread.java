class Workers {
    private final Object lock = new Object();

    void wrong(Thread worker) throws InterruptedException {
        synchronized (worker) {
            worker.wait();                                  // expect: wait-notify-on-thread
            worker.notifyAll();                             // expect: wait-notify-on-thread
        }
    }

    static class Sweeper extends Thread {
        private volatile boolean stopped;

        @Override
        public void run() {
            while (!stopped) {
                synchronized (this) {
                    try {
                        wait(1000);                         // expect: wait-notify-on-thread
                    } catch (InterruptedException e) {
                        return;
                    }
                }
            }
        }

        synchronized void shutdown() {
            stopped = true;
            this.notifyAll();                               // expect: wait-notify-on-thread
        }
    }

    void fine(Thread worker, Object other) throws InterruptedException {
        worker.join();
        synchronized (lock) {
            lock.wait(100);
            lock.notifyAll();
        }
        synchronized (other) {
            other.notify();
        }
    }

    static class Task implements Runnable {
        @Override
        public synchronized void run() {
            notifyAll();
        }
    }

    static class Owner extends Thread {
        private final Object gate = new Object();

        void await() throws InterruptedException {
            synchronized (gate) {
                gate.wait();
            }
            Runnable r = new Runnable() {
                @Override
                public synchronized void run() {
                    notify();
                }
            };
        }
    }
}
