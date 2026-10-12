import java.util.Arrays;
import java.util.List;

class Arrays2 {
    private byte[] data;
    private String[][] grid;

    String positives(int[] ids, String[] names, char[] chars, Object[] objs, List<String> list) {
        String a = ids.toString();                          // expect: array-to-string
        String b = "ids: " + ids;                           // expect: array-to-string
        String c = String.valueOf(names);                   // expect: array-to-string
        String d = data + " bytes";                         // expect: array-to-string
        String e = "chars: " + chars;                       // expect: array-to-string
        String f = "row " + grid[0];                        // expect: array-to-string
        String g = "" + list.toArray();                     // expect: array-to-string
        String h = "x";
        h += objs;                                          // expect: array-to-string
        String i = this.data.toString();                    // expect: array-to-string
        return a + b + c + d + e + f + g + h + i;
    }

    String more(String... parts) {
        int legacy[] = {1, 2};
        String a = "legacy " + legacy;                      // expect: array-to-string
        String b = "parts " + parts;                        // expect: array-to-string
        String c = "codes " + codes();                      // expect: array-to-string
        String d = "late " + declaredLater;                 // expect: array-to-string
        String e = "split " + a.split(",");                 // expect: array-to-string
        return a + b + c + d + e;
    }

    long[] codes() { return new long[0]; }

    private double[] declaredLater;

    String negatives(int[] ids, char[] chars, Unknown u, List<String> list) {
        String a = Arrays.toString(ids);
        String b = String.valueOf(chars);                   // char[] overload returns the text
        String c = new String(chars);
        String d = "n=" + ids.length;
        String e = "first=" + ids[0];
        String f = "u=" + u.values();                       // unknown type
        String g = list.toString();
        String h = "" + list;
        int s = ids[0] + ids[1];
        String k = Arrays.deepToString(grid);
        return a + b + c + d + e + f + g + h + s + k;
    }
}
