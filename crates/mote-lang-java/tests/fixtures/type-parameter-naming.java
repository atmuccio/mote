import java.util.List;
import java.util.Map;

class Cache<KeyType, V> {                                           // expect: type-parameter-naming
    <Item> List<Item> copy(List<Item> items) {                      // expect: type-parameter-naming
        return items;
    }

    <T, R> R apply(T t) {
        return null;
    }

    <T1, T2> void pair(T1 a, T2 b) { }

    <ID extends Comparable<ID>> void byId(ID id) { }                // expect: type-parameter-naming

    <t> void lower(t x) { }                                         // expect: type-parameter-naming

    <T10> void twoDigits() { }                                      // expect: type-parameter-naming
}

interface Repo<E, K extends Comparable<K>> {
    Map<K, E> all();
}

record Box<T>(T value) {
}

class Annotated<@Deprecated T> {
}
