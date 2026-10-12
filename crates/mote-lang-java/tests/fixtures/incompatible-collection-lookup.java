import java.time.LocalDate;
import java.util.ArrayList;
import java.util.Collection;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.UUID;

class Lookups {
    private final Map<String, Integer> counts = new HashMap<>();
    private final Set<Long> ids = new HashSet<>();

    void positives(int id, Integer boxed, UUID uuid, List<String> names, Map<Long, String> byId,
                   Collection<LocalDate> dates, String text) {
        ids.contains(id);                                           // expect: incompatible-collection-lookup
        ids.remove(boxed);                                          // expect: incompatible-collection-lookup
        counts.get(uuid);                                           // expect: incompatible-collection-lookup
        counts.containsKey(42);                                     // expect: incompatible-collection-lookup
        counts.containsValue("x");                                  // expect: incompatible-collection-lookup
        counts.getOrDefault(id, 0);                                 // expect: incompatible-collection-lookup
        counts.remove(uuid);                                        // expect: incompatible-collection-lookup
        names.indexOf(id);                                          // expect: incompatible-collection-lookup
        byId.get(id);                                               // expect: incompatible-collection-lookup
        dates.contains(text);                                       // expect: incompatible-collection-lookup
        this.ids.contains("7");                                     // expect: incompatible-collection-lookup
    }

    <T> void negatives(long lid, int index, Object o, CharSequence cs, List<String> names,
                       List<Integer> numbers, Map<CharSequence, String> byText, Set<Number> nums,
                       Map<String, List<String>> groups, T generic, Unknown u, Set<?> wild,
                       List<Object> objects, Set raw) {
        ids.contains(lid);                                          // long boxes to Long
        ids.contains(o);                                            // Object may be a Long
        names.remove(index);                                        // remove by index
        numbers.remove(index);                                      // remove by index
        numbers.contains(index);                                    // int boxes to Integer
        names.contains(cs);                                         // CharSequence may be a String
        byText.get("key");                                          // String is a CharSequence
        nums.contains(lid);                                         // Long is a Number
        groups.get("k");
        names.contains(generic);                                    // type variable: unknown
        names.contains(u);                                          // unknown type
        wild.contains(1);                                           // wildcard element type
        objects.contains(1);
        raw.contains(1);                                            // raw type
        names.get(index);                                           // List.get(int)
        counts.get(null);
        "abc".indexOf("b");
        new StringBuilder().indexOf("x");
    }
}
