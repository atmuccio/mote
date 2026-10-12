import java.util.List;
import java.util.function.BiFunction;
import java.util.function.Function;
import java.util.function.Supplier;

class Streams {
    private final Ledger ledger = new Ledger();
    private Ledger mutable = new Ledger();

    void positives(List<String> names, List<Order> orders, Handler handler) {
        names.forEach(n -> System.out.println(n));                  // expect: lambda-could-be-method-reference
        names.forEach(n -> handler.handle(n));                      // expect: lambda-could-be-method-reference
        names.forEach(n -> { handle(n); });                         // expect: lambda-could-be-method-reference
        orders.stream().map(o -> o.getTotal());                     // expect: lambda-could-be-method-reference
        names.stream().map(s -> new Label(s));                      // expect: lambda-could-be-method-reference
        BiFunction<Integer, Integer, Integer> f = (a, b) -> Math.max(a, b); // expect: lambda-could-be-method-reference
        Function<String, Integer> p = x -> Integer.parseInt(x);     // expect: lambda-could-be-method-reference
        Runnable r = () -> this.flush();                            // expect: lambda-could-be-method-reference
        names.forEach(n -> ledger.add(n));                          // expect: lambda-could-be-method-reference
        Supplier<Label> s = () -> { return new Label(); };          // expect: lambda-could-be-method-reference
    }

    void negatives(List<String> names, List<Integer> ints, Handler handler) {
        names.forEach(n -> handle(n, 1));                           // extra argument
        names.forEach(n -> handle(n.trim()));                       // transformed argument
        BiFunction<String, String, String> c = (a, b) -> concat(b, a); // order changed
        names.forEach(n -> getHandler().handle(n));                 // receiver computed at each call
        names.forEach(n -> mutable.add(n));                         // non-final field
        ints.stream().map(i -> i.toString());                       // Integer::toString is ambiguous
        ints.stream().map(i -> Integer.toString(i));                // ambiguous
        names.forEach(n -> new Object() { });                       // anonymous class
        names.forEach(n -> { handle(n); handle(n); });
        names.forEach(n -> Streams.<String>generic(n));             // explicit type arguments
        Runnable r = () -> { };
        names.forEach(n -> n.foo(n));                               // parameter used twice
        names.forEach(n -> unknown.handle(n));                      // unknown receiver
    }

    void handle(String s) { }

    void handle(String s, int i) { }

    String concat(String a, String b) { return a + b; }

    Handler getHandler() { return null; }

    void flush() { }

    static <T> void generic(T t) { }
}
