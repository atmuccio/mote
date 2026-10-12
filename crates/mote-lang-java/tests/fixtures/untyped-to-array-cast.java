import java.util.ArrayList;
import java.util.Collection;
import java.util.List;
import java.util.Set;
import java.util.stream.Stream;

class Arrays2<T> {
    private final List<String> names = new ArrayList<>();

    void wrong(Set<Integer> ids, Stream<String> words, Collection<Long> values) {
        String[] a = (String[]) names.toArray();                 // expect: untyped-to-array-cast
        Integer[] b = (Integer[]) ids.toArray();                 // expect: untyped-to-array-cast
        String[] c = (String[]) words.toArray();                 // expect: untyped-to-array-cast
        Long[] d = (Long[]) (values.toArray());                  // expect: untyped-to-array-cast
    }

    void fine(List<T> items, Unknown thing, Custom custom) {
        String[] a = names.toArray(new String[0]);
        Object[] b = (Object[]) names.toArray();
        T[] c = (T[]) items.toArray();
        String[] d = (String[]) thing.toArray();
        String[] e = (String[]) custom.toArray();
        String[] f = (String[]) names.toArray(new String[names.size()]);
    }

    static class Custom {
        String[] toArray() { return new String[0]; }
    }
}
