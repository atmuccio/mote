import java.util.Comparator;

class Compare {
    boolean positives(String a, String b, Comparator<String> cmp, Integer x, Integer y) {
        boolean r1 = a.compareTo(b) == 1;                   // expect: compare-to-specific-value
        boolean r2 = a.compareTo(b) == -1;                  // expect: compare-to-specific-value
        boolean r3 = cmp.compare(a, b) != 1;                // expect: compare-to-specific-value
        boolean r4 = -1 != x.compareTo(y);                  // expect: compare-to-specific-value
        boolean r5 = (a.compareTo(b)) == (-1);              // expect: compare-to-specific-value
        boolean r6 = Integer.compare(x, y) == 1;            // expect: compare-to-specific-value
        return r1 && r2 && r3 && r4 && r5 && r6;
    }

    boolean negatives(String a, String b, Comparator<String> cmp, Other o) {
        boolean n1 = a.compareTo(b) > 0;
        boolean n2 = a.compareTo(b) == 0;
        boolean n3 = cmp.compare(a, b) < 0;
        boolean n4 = a.compareTo(b) == 2;
        boolean n5 = o.compare(a) == 1;                     // not the two-argument compare
        boolean n6 = a.length() == 1;
        boolean n7 = o.compareTo(a, b) == -1;               // not the one-argument compareTo
        return n1 && n2 && n3 && n4 && n5 && n6 && n7;
    }
}
