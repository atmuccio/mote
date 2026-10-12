import java.io.IOException;
import java.util.concurrent.Callable;

class Throwing {
    void load() throws Exception { }                           // expect: throws-generic-exception
    void any() throws Throwable { }                            // expect: throws-generic-exception
    void both() throws IOException, java.lang.Exception { }    // expect: throws-generic-exception
    Throwing() throws Exception { }                            // expect: throws-generic-exception
    <T> T generic(Callable<T> c) throws Exception { return c.call(); }   // expect: throws-generic-exception

    interface Api {
        void execute() throws Exception;                       // expect: throws-generic-exception
    }

    // negatives
    public static void main(String[] args) throws Exception { }
    void specific() throws IOException { }
    @Override public String toString() { return ""; }
    @Override protected void finalize() throws Throwable { }
    public Object call() throws Exception { return null; }     // Callable-shaped
    public void close() throws Exception { }                   // AutoCloseable-shaped
    void none() { }

    Runnable r = new Runnable() {
        public void run() { }
        public void helper() throws Exception { }              // anonymous class member
    };

    Callable<String> c = new Callable<String>() {
        public String call() throws Exception { return "x"; }
    };

    @org.junit.Before
    public void setUp() throws Exception { }
}

class OwnException {
    static class Exception extends RuntimeException {}
    void m() throws Exception { }                              // project type named Exception
}
