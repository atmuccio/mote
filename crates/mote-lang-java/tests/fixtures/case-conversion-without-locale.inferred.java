import java.lang.management.ManagementFactory;
import java.lang.management.MemoryPoolMXBean;
import java.nio.file.Path;
import java.util.List;
import java.util.stream.Collectors;

class Names {
    enum State {
        CREATE, DONE;

        String extension() {
            return name().toLowerCase();                            // expect: case-conversion-without-locale
        }
    }

    List<String> find(Path f, Unknown u, List<String> names, List<Item> items, String query) {
        String a = f.toString().toLowerCase();                      // expect: case-conversion-without-locale
        String b = u.toString().toUpperCase();                      // expect: case-conversion-without-locale
        for (MemoryPoolMXBean pool : ManagementFactory.getMemoryPoolMXBeans()) {
            String c = pool.getName().toLowerCase();                // expect: case-conversion-without-locale
        }
        String d = u.name().toLowerCase();                          // unknown receiver: maybe not a String
        String e = items.get(0).name().toLowerCase();              // unknown type
        items.forEach(i -> i.toLowerCase());                        // elements of an unknown type
        return names.stream()
                .filter(n -> n.toLowerCase().startsWith(query))     // expect: case-conversion-without-locale
                .collect(Collectors.toList());
    }

    String name() {
        return "x";
    }

    String own() {
        return name().toLowerCase();                                // expect: case-conversion-without-locale
    }
}
