// mote-test: params java_release=8
import java.util.Comparator;
import java.util.List;
import java.util.concurrent.Callable;
import java.util.concurrent.Executor;
import java.util.function.Function;
import java.util.function.Predicate;

class Callbacks {
    interface Handler {
        void handle(String message);
        default void close() {}
    }

    interface Pair {
        void first();
        void second();
    }

    private String prefix;

    void positives(Executor executor, List<String> names, Cache cache) {
        executor.execute(new Runnable() {                               // expect: anonymous-class-could-be-lambda
            @Override
            public void run() {
                cache.refresh();
            }
        });
        names.sort(new Comparator<String>() {                         // expect: anonymous-class-could-be-lambda
            @Override
            public int compare(String a, String b) {
                return a.length() - b.length();
            }
        });
        Predicate<String> empty = new Predicate<String>() {            // expect: anonymous-class-could-be-lambda
            public boolean test(String s) {
                return s.isEmpty();
            }
        };
        Handler h = new Handler() {                                    // expect: anonymous-class-could-be-lambda
            @Override
            public void handle(String message) {
                System.out.println(prefix + message + Callbacks.this.prefix);
            }
        };
        register(new Callable<Integer>() {                              // expect: anonymous-class-could-be-lambda
            @Override
            public Integer call() throws Exception {
                return names.size();
            }
        });
    }

    Function<String, Integer> factory() {
        return new Function<String, Integer>() {                        // expect: anonymous-class-could-be-lambda
            @Override
            public Integer apply(String s) {
                return s.length();
            }
        };
    }

    void negatives(Executor executor, List<Object> things, String message) {
        // A field.
        executor.execute(new Runnable() {
            private int count;

            @Override
            public void run() {
                count++;
            }
        });
        // Uses this.
        executor.execute(new Runnable() {
            @Override
            public void run() {
                executor.execute(this);
            }
        });
        // Calls an inherited method.
        executor.execute(new Runnable() {
            @Override
            public void run() {
                System.out.println(toString());
            }
        });
        // Calls a default method of the interface.
        Handler h = new Handler() {
            @Override
            public void handle(String m) {
                close();
            }
        };
        // Recursion.
        Function<Integer, Integer> fact = new Function<Integer, Integer>() {
            @Override
            public Integer apply(Integer n) {
                return n <= 1 ? 1 : n * apply(n - 1);
            }
        };
        // The parameter name is taken by the enclosing method.
        Handler h2 = new Handler() {
            @Override
            public void handle(String message) {
                System.out.println(message);
            }
        };
        // No target type for a lambda.
        Object o = new Runnable() {
            @Override
            public void run() {
                System.out.println("x");
            }
        };
        things.add(new Runnable() {
            @Override
            public void run() {
                System.out.println("x");
            }
        });
        // Not a functional interface.
        Pair p = new Pair() {
            public void first() {}

            public void second() {}
        };
        // An abstract class.
        Task t = new Task() {
            @Override
            public void run() {
                System.out.println("x");
            }
        };
        // Other annotations.
        executor.execute(new Runnable() {
            @Override
            @SuppressWarnings("unchecked")
            public void run() {
                System.out.println("x");
            }
        });
        // Doubly privileged: ambiguous as a lambda.
        java.security.AccessController.doPrivileged(new java.security.PrivilegedAction<Void>() {
            @Override
            public Void run() {
                return null;
            }
        });
    }

    abstract static class Task implements Runnable {}

    void register(Callable<Integer> c) {}
}
