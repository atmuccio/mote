import java.lang.reflect.Method;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.text.MessageFormat;
import java.util.Arrays;
import java.util.List;
import java.util.stream.IntStream;
import java.util.stream.Stream;

class Calls {
    void jdk(String name, int count, Class<?> type, Method m, Object target, Object[] args, MessageFormat mf) throws Exception {
        String a = String.format("%s: %d", new Object[] { name, count });   // expect: explicit-varargs-array
        String b = String.format("%s", new Object[] { name });              // expect: explicit-varargs-array
        List<String> l = Arrays.asList(new String[] { "x", "y" });          // expect: explicit-varargs-array
        List<String> l2 = List.of(new String[] { name, name });             // expect: explicit-varargs-array
        Stream<String> s = Stream.of(new String[] { "x", "y" });            // expect: explicit-varargs-array
        Path p = Paths.get("root", new String[] { "a", "b" });              // expect: explicit-varargs-array
        Method run = type.getMethod("run", new Class[0]);                   // expect: explicit-varargs-array
        Method run2 = type.getMethod("run", new Class<?>[] { String.class }); // expect: explicit-varargs-array
        m.invoke(target, new Object[0]);                                    // expect: explicit-varargs-array
        m.invoke(target, new Object[] { name, count });                     // expect: explicit-varargs-array
        String c = MessageFormat.format("{0}", new Object[] { name, name }); // expect: explicit-varargs-array
        System.out.printf("%s %s%n", new Object[] { name, name });          // expect: explicit-varargs-array
        IntStream is = IntStream.of(new int[] { 1, 2 });                    // expect: explicit-varargs-array

        // A single element that could be an array or null: unwrapping changes the meaning.
        m.invoke(target, new Object[] { args });
        m.invoke(target, new Object[] { null });
        m.invoke(target, new Object[] { unknown() });                       // expect: explicit-varargs-array
        m.invoke(target, new Object[] { lib.get() });
        Paths.get("root", new String[] { null });
        // Primitive arrays to Object... / T...: one element there.
        List<int[]> ints = Arrays.asList(new int[] { 1, 2 });
        String d = String.format("%s", new int[] { 1 });
        // Not the last argument, a sized array, a two-dimensional array, a variable.
        Object[] made = new Object[] { name };
        String e = String.format("%s", new Object[2]);
        List<String[]> nested = Arrays.asList(new String[][] { { "a" } });
        String f = String.format("%s", args);
        // Instance MessageFormat.format(Object) takes an array.
        String g = mf.format(new Object[] { name, name });
        // Library methods: unknown signature.
        Logger.info("{} {}", new Object[] { name, name });
    }

    static Object unknown() { return null; }
}

class Local {
    void log(String fmt, Object... args) { }
    void names(String... names) { }
    void ints(int... values) { }

    void use(String a) {
        log("x", new Object[] { a, a });                                    // expect: explicit-varargs-array
        this.names(new String[] { a });                                     // expect: explicit-varargs-array
        ints(new int[] { 1, 2, 3 });                                        // expect: explicit-varargs-array
        log("x", new Object[] { a });                                       // expect: explicit-varargs-array
        log("x", new Object[] { new Object[0] });
        names(new String[] { null });
    }
}

class Overloaded {
    void log(String fmt, Object... args) { }
    void log(String fmt, Object arg) { }

    void use(String a) {
        log("x", new Object[] { a, a });
    }
}

class Inherits extends Base {
    void log(String fmt, Object... args) { }

    void use(String a) {
        log("x", new Object[] { a, a });
    }
}

class Outer {
    void log(String fmt, Object... args) { }

    class Inner extends Base {
        void use(String a) {
            log("x", new Object[] { a, a });
        }
    }
}
