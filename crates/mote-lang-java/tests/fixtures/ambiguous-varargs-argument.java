import java.lang.reflect.Method;
import java.util.Arrays;
import java.util.List;
import java.util.Set;
import java.util.stream.Stream;

class AmbiguousVarargsArgument {
    void log(String message, Object... args) {
        System.out.println(message + args.length);
    }

    void join(String... parts) {
        for (String p : parts) { }
    }

    void safe(Object... args) {
        if (args == null) {
            return;
        }
    }

    void over(Object... args) { }
    void over(String s, Object... args) { }

    void wrong(int[] ids, byte[] data, char[] letters) {
        log("value: %s", null);                                      // expect: ambiguous-varargs-argument
        this.join(null);                                             // expect: ambiguous-varargs-argument
        List<Object> a = Arrays.asList(null);                         // expect: ambiguous-varargs-argument
        boolean b = Arrays.asList(ids).contains(3);                     // expect: ambiguous-varargs-argument
        int c = List.of(data).size();                                   // expect: ambiguous-varargs-argument
        for (Object o : Set.of(letters)) { }                            // expect: ambiguous-varargs-argument
        long e = Stream.of(new int[] {1, 2}).count();                   // expect: ambiguous-varargs-argument
    }

    // Compliant
    void fine(Integer[] boxed, String[] names, Method m, Other other, int[] ids, byte[] data) throws Exception {
        log("value: %s", (Object) null);
        log("value", null, null);
        join((String) null);
        safe(null);                                                  // the callee checks for null
        over(null);                                                  // overloaded
        List<Integer> a = Arrays.asList(boxed);
        List<String> b = Arrays.asList(names);
        List<String> c = Arrays.asList("x");
        m.invoke(this, null);                                        // null means no arguments
        other.log("x", null);                                        // another class
        Object o = Stream.of(names);
        List<int[]> d = Arrays.asList(new int[0], new int[1]);        // several arrays: intended
        List<int[]> rows = Arrays.asList(ids);                        // a list of one array, as declared
        List<byte[]> chunks = List.of(data);
        Stream<int[]> s = Stream.of(ids);
        for (int[] row : List.of(ids)) { }
        for (var row : List.of(ids)) { }
        Object first = Arrays.asList(ids).get(0);
        java.util.function.Supplier<List<int[]>> lazy = () -> Arrays.asList(ids);
    }

    interface Other { void log(String s, Object... args); }

    static class Child extends Base {
        void run() {
            report(null);                                             // an inherited overload may exist
        }
        void report(Object... xs) { }
    }

    static class Base { }
}

class WithInterface implements Runnable {
    public void run() {
        send(null);                                                  // a default method could take the call
    }
    void send(Object... xs) { }
}
