class Box {
    private int width;
    private int height;
    private String first;
    private String last;

    public int getWidth() { return width; }

    public int getHeight() { return width; }                    // expect: accessor-uses-wrong-field

    public void setWidth(int w) { this.width = w; }

    public void setHeight(int h) { this.width = h; }            // expect: accessor-uses-wrong-field

    public String getFirst() { return first; }

    // expect-next-line: accessor-uses-wrong-field
    public String getLast() {
        return this.first;
    }

    public void setFirst(String first) { this.first = first; }

    public void setLast(String last) { this.last = last; }
}

class Container {
    private Listener listener;
    private Listener proxy;
    private Provider textProvider;
    private Provider defaultTextProvider;
    private Node next;
    private Node _next;
    private int size;
    private long count;
    private long total;

    // a getter deliberately exposing another field: no twin accessor for `proxy`
    public Listener getListener() { return proxy; }

    public void setTextProvider(Provider p) { this.defaultTextProvider = p; }

    public Node getNext() {
        return _next; // returns the internal link on purpose
    }

    public Node get_next() { return _next; }

    public long getCount() { return total; }                    // expect: accessor-uses-wrong-field

    public long getTotal() { return total; }

    public int getSize() { return size; }

    public boolean isEmpty() { return size == 0; }

    public void setSize(int size) { size = size; }

    interface Listener { }
    interface Provider { }
    interface Node { }
}
