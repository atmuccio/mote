import java.util.List;
import java.util.Map;

class StringEq {
    private String name;
    private static final String CONST = "x";

    boolean positives(String a, String b, List<String> list, Map<String, String> map, boolean flag) {
        boolean r1 = a == b;                          // expect: string-reference-equality
        boolean r2 = a != "literal";                  // expect: string-reference-equality
        boolean r3 = "x" == name;                     // expect: string-reference-equality
        boolean r4 = this.name == CONST;              // expect: string-reference-equality
        boolean r5 = list.get(0) == a;                // expect: string-reference-equality
        boolean r6 = map.get("k") != b.trim();        // expect: string-reference-equality
        var local = a.substring(1);
        boolean r7 = local == (b + "!");              // expect: string-reference-equality
        boolean r8 = String.valueOf(1) == a;          // expect: string-reference-equality
        boolean r9 = (flag ? a : b) == name;          // expect: string-reference-equality
        for (String s : list) {
            if (s == a) { return true; }              // expect: string-reference-equality
        }
        for (var e : map.entrySet()) {
            if (e.getKey() == a) { return true; }     // expect: string-reference-equality
        }
        return r1 && r2 && r3 && r4 && r5 && r6 && r7 && r8 && r9;
    }

    boolean negatives(String a, Object o, Integer i, Integer j, CharSequence cs, Foo foo, List<String> list) {
        boolean n1 = a == null;
        boolean n2 = null != a;
        boolean n3 = o == a;                // Object vs String: an identity check on purpose
        boolean n4 = i == j;                // not strings
        boolean n5 = a.intern() == "x".intern();
        boolean n6 = cs == a;
        boolean n7 = foo.name() == a;       // receiver type unknown
        boolean n8 = a.equals("x");
        list.forEach(s -> check(s == a));   // lambda parameter type is not inferred
        List raw = list;
        boolean n9 = raw.get(0) == a;       // raw type: element type unknown
        return n1 && n2 && n3 && n4 && n5 && n6 && n7 && n8 && n9;
    }

    boolean literals(Order o, Object obj, CharSequence cs, Integer i) {
        // A literal on one side is enough evidence, even when the other side's type is unknown.
        boolean l1 = o.customerId() == "guest";       // expect: string-reference-equality
        boolean l2 = "admin" != inheritedName;        // expect: string-reference-equality
        boolean l3 = (o.name()) == ("x");             // expect: string-reference-equality
        boolean k1 = obj == "guest";                  // Object: an identity check on purpose
        boolean k2 = cs == "x";                       // known to be another type
        boolean k3 = "x" == null;
        return l1 && l2 && l3 && k1 && k2 && k3;
    }

    void shadowing() {
        Object name = new Object();
        boolean s = name == CONST;          // the local `name` is an Object, not the String field
    }

    <T> boolean generic(T x, T y) {
        return x == y;
    }

    static class Inner extends Base {
        boolean inherited(String a) {
            return value == a;              // `value` may be inherited from an unknown superclass
        }
    }
}
