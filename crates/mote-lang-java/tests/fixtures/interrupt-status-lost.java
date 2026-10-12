import java.util.concurrent.BlockingQueue;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.ExecutionException;
import java.util.concurrent.Future;
import java.util.logging.Logger;

class Worker {
    private static final Logger LOG = Logger.getLogger("w");
    private volatile boolean running = true;
    private int failures;
    private InterruptedException failure;

    void positives(BlockingQueue<Runnable> queue, Future<String> future) {
        while (running) {
            try {
                queue.take().run();
            } catch (InterruptedException e) {                       // expect: interrupt-status-lost
                LOG.warning("interrupted: " + e.getMessage());
            }
        }
        try {
            Thread.sleep(100);
        } catch (InterruptedException e) {                           // expect: interrupt-status-lost
            e.printStackTrace();
        }
        try {
            future.get();
        } catch (ExecutionException | InterruptedException e) {      // expect: interrupt-status-lost
            LOG.log(java.util.logging.Level.WARNING, "failed", e);
            failures++;
        }
        try {
            Thread.sleep(10);
        } catch (java.lang.InterruptedException e) {                 // expect: interrupt-status-lost
            System.err.println("woken early " + e);
        }
        for (int i = 0; i < 3; i++) {
            try {
                Thread.sleep(10);
            } catch (InterruptedException e) {                       // expect: interrupt-status-lost
                LOG.fine("retrying");
                continue;
            }
        }
        Runnable r = () -> {
            try {
                Thread.sleep(5);
            } catch (InterruptedException ex) {                      // expect: interrupt-status-lost
                failures++;
            }
        };
    }

    void restores(BlockingQueue<Runnable> queue) {
        try {
            queue.take().run();
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            LOG.warning("stopping");
        }
    }

    void rethrows(BlockingQueue<Runnable> queue) {
        try {
            queue.take().run();
        } catch (InterruptedException e) {
            LOG.warning("stopping");
            throw new IllegalStateException("interrupted", e);
        }
    }

    void handsOver(Future<String> future, CompletableFuture<String> cf) {
        try {
            future.get();
        } catch (InterruptedException | ExecutionException e) {
            cf.completeExceptionally(e);
        }
        try {
            future.get();
        } catch (InterruptedException e) {
            handleInterrupt(e);
        }
        try {
            future.get();
        } catch (InterruptedException e) {
            failure = e;
        }
    }

    void emptyOrExit() {
        try {
            Thread.sleep(1);
        } catch (InterruptedException ignored) {
            // empty handlers belong to empty-catch-block
        }
        try {
            Thread.sleep(1);
        } catch (InterruptedException e) {
            System.exit(1);
        }
    }

    void stops(BlockingQueue<Runnable> queue) {
        while (running) {
            try {
                queue.take().run();
            } catch (InterruptedException e) {
                LOG.info("worker stopping");
                break;
            }
        }
        boolean interrupted = false;
        try {
            queue.take().run();
        } catch (InterruptedException e) {
            interrupted = true;
        }
        if (interrupted) {
            throw new IllegalStateException("interrupted");
        }
    }

    boolean awaitDone(java.util.concurrent.CountDownLatch latch) {
        try {
            latch.await();
            return true;
        } catch (InterruptedException e) {
            LOG.info("gave up waiting");
            return false;
        }
    }

    class Loader extends javax.swing.SwingWorker<String, Void> {
        protected String doInBackground() { return "x"; }

        protected void done() {
            try {
                show(get());
            } catch (InterruptedException | ExecutionException e) {
                LOG.warning("failed");
            }
        }

        void show(String s) {}
    }

    class Mirror extends Thread {
        public void run() {
            try {
                Thread.sleep(5);
            } catch (InterruptedException e) {
                LOG.warning("mirror interrupted");
            }
            LOG.fine("end of thread");
        }
    }

    class Poller extends Thread {
        public void run() {
            while (running) {
                try {
                    Thread.sleep(5);
                } catch (InterruptedException e) {                   // expect: interrupt-status-lost
                    LOG.warning("poller interrupted");
                }
            }
        }
    }

    void otherExceptions(Future<String> future) {
        try {
            future.get();
        } catch (ExecutionException e) {
            LOG.warning("failed");
        } catch (Exception e) {
            LOG.warning("other");
        }
    }

    String uninterruptibly(Future<String> future) throws ExecutionException {
        boolean interrupted = false;
        try {
            while (true) {
                try {
                    return future.get();
                } catch (InterruptedException e) {
                    interrupted = true;
                }
            }
        } finally {
            if (interrupted) {
                Thread.currentThread().interrupt();
            }
        }
    }

    void handleInterrupt(InterruptedException e) {
        Thread.currentThread().interrupt();
    }
}
