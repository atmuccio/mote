import java.util.Arrays;
import java.util.List;
import java.util.stream.IntStream;
import java.util.stream.Stream;

class ArraysAsListStream {
    long count(String[] names) {
        return Arrays.asList(names).stream().filter(String::isBlank).count();   // expect: arrays-as-list-stream
    }

    Stream<String> values() {
        return Arrays.asList("a", "b", "c").stream();                           // expect: arrays-as-list-stream
    }

    long primitives(int[] numbers) {
        return Arrays.asList(numbers).stream().count();                         // expect: arrays-as-list-stream
    }

    Stream<String> single(String name) {
        return java.util.Arrays.asList(name).stream();                          // expect: arrays-as-list-stream
    }

    <T> Stream<T> generic(T[] items) {
        return Arrays.asList(items).stream();                                   // expect: arrays-as-list-stream
    }

    // Compliant
    void fine(String[] names, Object unknown, List<String> list) {
        Arrays.stream(names).count();
        Stream.of("a", "b").count();
        Arrays.asList(names).parallelStream().count();      // parallel on purpose
        Arrays.asList().stream().count();                   // no arguments
        Arrays.asList(unknownValue()).stream().count();     // type not known
        list.stream().count();
        Arrays.asList(names).forEach(System.out::println);
        IntStream.of(1, 2).count();
    }

    static class Arrays2 {
        static <T> List<T> asList(T... a) { return null; }
    }

    void notJdk(String[] names) {
        Arrays2.asList(names).stream().count();
    }
}
