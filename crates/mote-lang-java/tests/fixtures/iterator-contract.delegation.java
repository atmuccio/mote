import java.util.Iterator;
import java.util.Map;

class Counter implements Iterable<Integer> {
    private int start;

    // Infinite iterators: hasNext() is always true, so next() never runs out.
    @Override
    public Iterator<Integer> iterator() {
        return new Iterator<Integer>() {
            private int value = start;

            @Override
            public boolean hasNext() {
                return true;
            }

            @Override
            public Integer next() {
                return value++;
            }
        };
    }

    static Iterator<String> blanks() {
        return new Iterator<>() {
            @Override
            public boolean hasNext() {
                // never exhausted
                return (true);
            }

            @Override
            public String next() {
                return null;
            }
        };
    }

    // Not infinite: hasNext() is a real check.
    static Iterator<String> limited(int max) {
        return new Iterator<>() {
            private int n;

            @Override
            public boolean hasNext() {
                return n < max;
            }

            @Override
            public String next() {                              // expect: iterator-contract
                return "item" + n++;
            }
        };
    }
}

class EntryView<K, V> implements Iterable<Map.Entry<K, V>> {
    private final Map<K, V> map;

    EntryView(Map<K, V> map) { this.map = map; }

    // next() delegates through a field initializer of the object it returns.
    @Override
    public Iterator<Map.Entry<K, V>> iterator() {
        return new Iterator<>() {
            private final Iterator<Map.Entry<K, V>> inner = map.entrySet().iterator();

            @Override
            public boolean hasNext() {
                return inner.hasNext();
            }

            @Override
            public Map.Entry<K, V> next() {
                return new Map.Entry<>() {
                    private final Map.Entry<K, V> current = inner.next();

                    @Override
                    public K getKey() { return current.getKey(); }

                    @Override
                    public V getValue() { return current.getValue(); }

                    @Override
                    public V setValue(V value) { return current.setValue(value); }
                };
            }
        };
    }

    // The delegation sits in a method of the returned object: it runs later, not in next().
    Iterator<Map.Entry<K, V>> lazy() {
        Iterator<Map.Entry<K, V>> inner = map.entrySet().iterator();
        return new Iterator<>() {
            @Override
            public boolean hasNext() {
                return inner.hasNext();
            }

            @Override
            public Map.Entry<K, V> next() {                     // expect: iterator-contract
                return new Map.Entry<>() {
                    @Override
                    public K getKey() { return inner.next().getKey(); }

                    @Override
                    public V getValue() { return null; }

                    @Override
                    public V setValue(V value) { return null; }
                };
            }
        };
    }
}
