import java.awt.event.ActionEvent;
import java.awt.event.ActionListener;
import java.awt.event.MouseAdapter;
import java.awt.event.MouseEvent;
import java.io.Closeable;
import java.io.IOException;
import java.io.InputStream;
import java.util.AbstractMap;
import java.util.Comparator;
import java.util.Iterator;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.Callable;
import java.util.function.Function;
import javax.swing.table.AbstractTableModel;
import org.xml.sax.Attributes;
import org.xml.sax.helpers.DefaultHandler;
import com.acme.Widget;
import com.acme.Handler;

class Task implements Runnable, Callable<String>, Comparable<Task>, Closeable {
    int rank;

    public void run() { }                                          // expect: missing-override
    public String call() { return "done"; }                        // expect: missing-override
    public int compareTo(Task other) { return rank - other.rank; } // expect: missing-override
    public void close() throws IOException { }                     // expect: missing-override

    public int compareTo(String other) { return 0; }               // overload: different type
    public void run(int times) { }                                 // overload: different arity
    public static void close(Task t) { }                           // static: overrides nothing
    private String call(int retries) { return ""; }                // private
    @Override
    public boolean equals(Object o) { return o == this; }
    @java.lang.Override
    public int hashCode() { return rank; }
}

class Worker extends Thread {
    public void run() { }                                          // expect: missing-override
    public void start(int delay) { }
}

class Boom extends RuntimeException {
    public String getMessage() { return "boom"; }                  // expect: missing-override
    public String getMessage(int code) { return "boom " + code; }
}

class Numbers implements Iterator<Integer>, Iterable<Integer> {
    public boolean hasNext() { return false; }                     // expect: missing-override
    public Integer next() { return 1; }                            // expect: missing-override
    public void remove() { }                                       // expect: missing-override
    public Iterator<Integer> iterator() { return this; }           // expect: missing-override
}

class ByLength implements Comparator<String> {
    public int compare(String a, String b) { return a.length() - b.length(); } // expect: missing-override
}

class Registry extends AbstractMap<String, Integer> {
    public Set<Map.Entry<String, Integer>> entrySet() { return null; } // expect: missing-override
    public Integer put(String key, Integer value) { return value; }    // expect: missing-override
    public Integer put(Integer key, String value) { return null; }     // not an override: other types
}

class Source extends InputStream {
    public int read() { return -1; }                               // expect: missing-override
    public int read(byte[] b, int off, int len) { return -1; }     // expect: missing-override
    public int read(char[] c) { return -1; }                       // no such method in InputStream
}

class Ui {
    ActionListener onClick = new ActionListener() {
        public void actionPerformed(ActionEvent e) { }             // expect: missing-override
    };
    MouseAdapter onPress = new MouseAdapter() {
        public void mousePressed(MouseEvent e) { }                 // expect: missing-override
        public void mousePressed(ActionEvent e) { }                // wrong parameter type
    };
    Function<String, Integer> length = new Function<>() {
        public Integer apply(String s) { return s.length(); }      // expect: missing-override
    };
    Comparator<Integer> desc = new Comparator<Integer>() {
        @Override
        public int compare(Integer a, Integer b) { return b - a; }
    };
}

class Rows extends AbstractTableModel {
    public int getRowCount() { return 0; }                         // expect: missing-override
    public int getColumnCount() { return 0; }                      // expect: missing-override
    public Object getValueAt(int row, int col) { return null; }    // expect: missing-override
    public Object getValueAt(long row) { return null; }
}

class Parser extends DefaultHandler {
    public void startElement(String uri, String local, String qName, Attributes atts) { } // expect: missing-override
    public void characters(char[] ch, int start, int length) { }   // expect: missing-override
    public void finish() { }
}

// Unknown supertypes: nothing can be said about their methods.
class Custom extends Widget implements Handler {
    public void render() { }
    public void handle(String event) { }
    public String toString() { return "custom"; }                  // expect: missing-override
}

interface Job extends Runnable {
    default void run() { }                                         // interface member: not checked
}

abstract class Partial implements Runnable {
    public abstract void run();                                    // no body
}

class Generic<T> implements Comparable<Generic<T>> {
    public int compareTo(Generic<T> o) { return 0; }               // expect: missing-override
}

class Varargs implements java.util.function.Consumer<String[]> {
    public void accept(String... values) { }                       // expect: missing-override
}

// Type arguments from other files are compared by name.
class Statuses extends java.util.ArrayList<Widget> {
    public Widget add(String id) { return null; }                  // overload: `Widget` is not `String`
    public boolean add(Handler h) { return false; }                // overload: another unknown type
    public boolean add(Widget w) { return true; }                  // expect: missing-override
}

class ByWidget implements Comparator<Widget> {
    public int compare(Widget a, Widget b) { return 0; }           // expect: missing-override
}

@SuppressWarnings("rawtypes")
class Legacy implements Comparable {
    public int compareTo(Object o) { return 0; }                   // expect: missing-override
}

class Sink<T> implements java.util.function.Consumer<T> {
    public void accept(T value) { }                                // expect: missing-override
    public <U> void accept(U value, int n) { }
}

class Copier<E> extends java.util.AbstractList<E> {
    public E get(int index) { return null; }                       // expect: missing-override
    public int size() { return 0; }                                // expect: missing-override
    public <T> T[] toArray(T[] a) { return a; }                    // expect: missing-override
}
