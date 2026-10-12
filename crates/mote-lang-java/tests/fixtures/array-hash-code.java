import java.util.Arrays;
import java.util.List;
import java.util.Objects;

class Key {
    private final byte[] data;
    private final String[] tags;
    private final int id;

    Key(byte[] data, String[] tags, int id) {
        this.data = data;
        this.tags = tags;
        this.id = id;
    }

    @Override
    public int hashCode() {
        int h = data.hashCode();                                    // expect: array-hash-code
        h = 31 * h + this.tags.hashCode();                          // expect: array-hash-code
        h = 31 * h + Objects.hashCode(tags);                        // expect: array-hash-code
        h = 31 * h + java.util.Objects.hashCode(data);              // expect: array-hash-code
        int[] local = {1, 2};
        h += local.hashCode();                                      // expect: array-hash-code
        return h + tags[0].hashCode() + id;
    }

    int fine(List<String> list, Object o, Unknown u) {
        return Arrays.hashCode(data)
                + Arrays.deepHashCode(tags)
                + Objects.hash(id, list)
                + list.hashCode()
                + o.hashCode()
                + u.values().hashCode()
                + Objects.hashCode(o)
                + super.hashCode();
    }
}
