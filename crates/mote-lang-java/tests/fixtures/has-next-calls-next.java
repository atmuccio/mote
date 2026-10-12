import java.util.Enumeration;
import java.util.Iterator;

class Lines implements Iterator<String> {
    private final Reader reader;

    Lines(Reader reader) { this.reader = reader; }

    @Override
    public boolean hasNext() {
        return next() != null;                                  // expect: has-next-calls-next
    }

    @Override
    public String next() {
        return reader.readLine();
    }

    interface Reader { String readLine(); }
}

class Tokens implements Enumeration<String> {
    public boolean hasMoreElements() {
        String t = this.nextElement();                          // expect: has-next-calls-next
        return t != null;
    }

    public String nextElement() {
        return "x";
    }
}

class Peeking implements Iterator<String> {
    private final Iterator<String> delegate;
    private String pending;

    Peeking(Iterator<String> delegate) { this.delegate = delegate; }

    @Override
    public boolean hasNext() {
        if (pending == null && delegate.hasNext()) {
            pending = delegate.next();
        }
        return pending != null;
    }

    @Override
    public String next() {
        String p = pending;
        pending = null;
        return p;
    }
}

class Wrapper {
    private final Iterator<String> it;

    Wrapper(Iterator<String> it) { this.it = it; }

    public boolean hasNext() {
        return next() != null;
    }

    String nextOne() { return it.next(); }

    static String next() { return null; }

    static String next(int i) { return null; }
}
