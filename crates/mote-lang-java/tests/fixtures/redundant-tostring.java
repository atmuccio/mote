import java.util.List;
import java.util.Map;

class ToStrings {
    private String name;

    String positives(String s, List<String> list, Map<String, String> map, StringBuilder sb) {
        String a = s.toString();                            // expect: redundant-tostring
        String b = name.trim().toString();                  // expect: redundant-tostring
        String c = list.get(0).toString();                  // expect: redundant-tostring
        String d = map.get("k").toString();                 // expect: redundant-tostring
        String e = "literal".toString();                    // expect: redundant-tostring
        String f = (s + 1).toString();                      // expect: redundant-tostring
        String g = sb.toString().toString();                // expect: redundant-tostring
        return a + b + c + d + e + f + g;
    }

    String negatives(Object o, StringBuilder sb, CharSequence cs, Unknown u, List<String> list) {
        String a = o.toString();
        String b = sb.toString();
        String c = cs.toString();
        String d = u.name().toString();                     // unknown type
        list.forEach(x -> System.out.println(x.toString())); // lambda parameter type is unknown
        return a + b + c + d + toString() + super.toString();
    }
}
