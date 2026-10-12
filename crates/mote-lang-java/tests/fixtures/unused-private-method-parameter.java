import java.io.IOException;
import java.io.ObjectInputStream;
import java.util.List;
import java.util.Locale;

class Labels {
    private String label(Order order, Locale locale) {              // expect: unused-private-method-parameter
        return "#" + order.id();
    }

    private void many(int a, int b, int c) {                        // expect: unused-private-method-parameter, unused-private-method-parameter
        System.out.println(b);
    }

    private void varargs(String first, String... rest) {            // expect: unused-private-method-parameter
        System.out.println(first);
    }

    private String used(Order order, Locale locale) {
        return order.id().toUpperCase(locale);
    }

    private void unusedByName(int unusedFlag, int ignoredValue) {
        System.out.println(1);
    }

    private void empty(int x) {
    }

    @SuppressWarnings("unused")
    private void annotated(int x) {
        System.out.println(1);
    }

    private void annotatedParam(@SuppressWarnings("unused") int x) {
        System.out.println(1);
    }

    private void readObject(ObjectInputStream in) throws IOException {
        System.out.println(1);
    }

    private void callback(String event) {                           // referenced as this::callback
        refresh();
    }

    private void reflective(String arg) {                           // named in a string: reflection
        refresh();
    }

    void register(List<String> events) {
        events.forEach(this::callback);
        invoke("reflective");
    }

    public void publicMethod(int x) {                               // not private
        refresh();
    }

    void packagePrivate(int x) {
        refresh();
    }

    private void usedInLambda(int x) {
        Runnable r = () -> System.out.println(x);
        r.run();
    }

    private void usedInComment(int x) {                             // expect: unused-private-method-parameter
        // x is only named in this comment
        refresh();
    }

    private abstract class Inner {
        private void inner(int y) {                                 // expect: unused-private-method-parameter
            refresh();
        }
    }

    void refresh() { }

    void invoke(String name) { }
}
