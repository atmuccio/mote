import java.util.ArrayDeque;
import java.util.Iterator;
import java.util.List;
import java.util.NoSuchElementException;

class ArrayIter<T> implements Iterator<T> {
    private final T[] items;
    private int position;

    ArrayIter(T[] items) { this.items = items; }

    public boolean hasNext() { return position < items.length; }

    public T next() {                                       // expect: iterator-contract
        return items[position++];
    }
}

class NullAtEnd implements Iterator<String>, Iterable<String> {
    private final List<String> lines;
    private int i;

    NullAtEnd(List<String> lines) { this.lines = lines; }

    public boolean hasNext() { return i < lines.size(); }

    public String next() {                                  // expect: iterator-contract
        if (!hasNext()) {
            return null;
        }
        return lines.get(i++);
    }

    public Iterator<String> iterator() {                    // expect: iterator-contract
        return this;
    }
}

class Factory {
    Iterator<Integer> counter(int max) {
        return new Iterator<Integer>() {
            int n;
            public boolean hasNext() { return n < max; }
            public Integer next() { return n++; }           // expect: iterator-contract
        };
    }

    // Negatives.
    Iterator<Integer> checked(int max) {
        return new Iterator<>() {
            int n;
            public boolean hasNext() { return n < max; }
            public Integer next() {
                if (n >= max) {
                    throw new NoSuchElementException();
                }
                return n++;
            }
        };
    }
}

class Delegating<T> implements Iterator<T> {
    private final Iterator<T> inner;
    private final ArrayDeque<T> queue = new ArrayDeque<>();

    Delegating(Iterator<T> inner) { this.inner = inner; }

    public boolean hasNext() { return inner.hasNext(); }

    public T next() { return inner.next(); }
}

class Queued<T> implements Iterator<T> {
    private final ArrayDeque<T> queue = new ArrayDeque<>();

    public boolean hasNext() { return !queue.isEmpty(); }

    public T next() { return queue.pop(); }
}

class Helper<T> implements Iterator<T> {
    private T current;

    public boolean hasNext() { return current != null; }

    public T next() {
        ensureMore();
        return current;
    }

    private void ensureMore() {
        if (current == null) throw new NoSuchElementException();
    }
}

class NotAnIterator {
    String next() { return "x"; }

    Iterable<String> iterator() { return null; }
}

class Fresh implements Iterable<String> {
    public Iterator<String> iterator() { return List.of("a").iterator(); }
}

class Codepoints {
    Iterator<String> of(java.util.PrimitiveIterator.OfInt iter) {
        return new Iterator<String>() {
            public boolean hasNext() { return iter.hasNext(); }
            public String next() { return new String(Character.toChars(iter.nextInt())); }
        };
    }
}
