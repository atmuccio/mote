class ThreadSubclassWithoutRun {
    final class Poller extends Thread {                            // expect: thread-subclass-without-run
        public void Run() { }
    }

    private static class Worker extends java.lang.Thread {         // expect: thread-subclass-without-run
        Worker(String name) { super(name); }
        void work(int n) { }
        public void run(int times) { }
    }

    static class Used extends Thread {                             // expect: thread-subclass-without-run
        void setup() { }
    }

    void start() {
        new Used().start();
        Thread t = new Thread() {                                  // expect: thread-subclass-without-run
            void work() { }
        };
        class Local extends Thread { }                             // expect: thread-subclass-without-run
        t.start();
    }

    // Compliant
    final class Good extends Thread {
        @Override
        public void run() { }
    }

    final class WithTarget extends Thread {
        WithTarget(Runnable r) { super(r); }
    }

    final class WithLambda extends Thread {
        WithLambda() { super(() -> System.out.println("hi")); }
    }

    final class WithStart extends Thread {
        @Override
        public synchronized void start() { }
    }

    abstract static class Base extends Thread { }

    static class Exported extends Thread { }                       // may be extended in another file

    static class Parent extends Thread { }                         // extended below
    final class Child extends Parent {
        @Override
        public void run() { }
    }

    void fine(Runnable r) {
        Thread a = new Thread(r) { };
        Thread b = new Thread() {
            @Override
            public void run() { }
        };
        new Parent().start();
    }

    final class NotAThread extends Object { }

    static class Thread2 { }
    final class OtherThread extends Thread2 { }
}

class ThreadTraps {
    private static class Base extends Thread { }                  // extended below with run()

    private static class Worker extends Base {
        @Override
        public void run() { }
    }

    static class Hooked extends Thread { }                          // only anonymous subclasses are created

    void start() {
        new Worker().start();
        new Hooked() {
            @Override
            public void run() { }
        }.start();
    }
}
