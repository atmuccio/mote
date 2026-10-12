import java.util.ArrayList;
import java.util.List;

class Throwaway {
    String positives(int count, long total, char c, boolean flag, int other, Integer boxedOther) {
        String a = Integer.valueOf(count).toString();                        // expect: boxing-roundtrip
        int h = Long.valueOf(total).hashCode();                              // expect: boxing-roundtrip
        int cmp = Integer.valueOf(count).compareTo(other);                   // expect: boxing-roundtrip
        int cmp2 = Integer.valueOf(count).compareTo(boxedOther);             // expect: boxing-roundtrip
        String b = Character.valueOf(c).toString();                          // expect: boxing-roundtrip
        String line = "total: " + String.valueOf(total);                     // expect: boxing-roundtrip
        String tail = String.valueOf(flag) + " flag";                        // expect: boxing-roundtrip
        Class<?> k = new ArrayList<String>().getClass();                     // expect: boxing-roundtrip
        return a + b + line + tail + h + cmp + cmp2 + k;
    }

    String negatives(String text, int count, long total, char[] chars, Object o, List<String> list) {
        String a = Integer.valueOf(text).toString();                         // parsed first: normalizes
        String b = Integer.toString(count);
        String c = "chars: " + String.valueOf(chars);                        // char[] overload
        String d = String.valueOf(count) + String.valueOf(total);            // both converted
        String e = count + String.valueOf(total);                            // int + ...: would add
        String f = String.valueOf(count);
        String g2 = "n=" + String.valueOf(count + 1);                          // an operator inside
        Class<?> g = new ArrayList<String>(list).getClass();                 // constructor with arguments
        Class<?> h = new Runnable() { public void run() {} }.getClass();     // anonymous class
        Class<?> i = o.getClass();
        int j = Integer.valueOf(count).compareTo(Integer.valueOf(3));        // expect: boxing-roundtrip
        long k = Long.valueOf(total).longValue();                            // expect: boxing-roundtrip
        return a + b + c + d + e + f + g + h + i + j + k;
    }
}
