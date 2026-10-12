import java.util.ArrayList;
import java.util.List;

class Order<T> {
    private int total;
    private volatile Thread worker;
    private String name;
    private static final int LIMIT = 10;
    protected List<T> lines = new ArrayList<>();

    Order(String name) {
        this.name = name;
        int total = 0;                                  // constructor: accepted
    }

    void add(int price, int quantity) {
        int total = price * quantity;                   // expect: hidden-declaration
        System.out.println(total);
    }

    Thread getWorker() {
        return worker;
    }

    void accessorCopy() {
        Thread worker = getWorker();                    // read through its own accessor
        String name = this.name();                      // likewise
    }

    String name() {
        return name;
    }

    void copies() {
        Thread worker = this.worker;                    // local copy of a volatile field
        if (worker != null) {
            worker.interrupt();
        }
    }

    void stages(String raw) {
        String name = raw.trim();                       // stored in the field afterwards
        this.name = name;
    }

    void limit() {
        int LIMIT = 3;                                  // static constant: accepted
        System.out.println(LIMIT);
    }

    static Order<String> create(int total) {
        Order<String> o = new Order<>("x");
        int name = 1;                                   // static method: accepted
        return o;
    }

    void lambdas() {
        Runnable r = () -> {
            int total = 1;                              // lambda: accepted
        };
        Runnable anon = new Runnable() {
            @Override
            public void run() {
                int total = 2;                          // anonymous class: accepted
            }
        };
    }

    void parameter(int total) {                         // parameters: accepted
        System.out.println(total);
    }

    <T> void generic(T value) {                         // expect: hidden-declaration
        System.out.println(value);
    }

    static <T> Order<T> of(T first) {                   // static: cannot see the class's T
        return null;
    }

    class Inner<T> {                                    // expect: hidden-declaration
    }

    static class Nested<T> {
    }

    <U> void other(U value) {
    }
}

class SpecialOrder extends Order<String> {
    protected List<String> lines = new ArrayList<>();   // expect: hidden-declaration
    private int total;                                  // private in the parent: accepted
    static final int LIMIT = 5;

    SpecialOrder() {
        super("special");
    }
}

interface Filter {
    boolean accept(String s);
}

class Filters {
    static class FileFilter implements java.io.FileFilter { // named like its supertype: accepted
        public boolean accept(java.io.File f) {
            return true;
        }
    }
}

record Point(int x, int y) {
    int shifted() {
        int x = this.x + 1;                             // copies the component
        return x;
    }

    int moved() {
        int y = 3;                                      // expect: hidden-declaration
        return y;
    }
}

class Appearance {
    private String fontName;
    private float fontSize;
    private int color;

    void apply(java.util.List<Object> operands) {
        String fontName = (String) operands.get(0);     // stored through the class's own setter
        float fontSize = 12f;                           // expect: hidden-declaration
        int color = 3;                                  // expect: hidden-declaration
        setFontName(fontName);
        other.setFontSize(fontSize);                    // another object's setter
        setColor(color);                                // a setter that does not store the field
    }

    void setFontName(String fontName) {
        this.fontName = fontName;
    }

    void setColor(int color) {
        log(color);
    }

    void log(int c) {
    }

    private Appearance other;
}
