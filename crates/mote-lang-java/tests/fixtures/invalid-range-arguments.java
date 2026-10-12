import java.util.Arrays;
import java.util.List;

class Ranges {
    void wrong(String date, List<String> items, int[] values, StringBuilder sb, int v) {
        String year = date.substring(4, 0);                   // expect: invalid-range-arguments
        String tail = date.substring(-1);                     // expect: invalid-range-arguments
        List<String> head = items.subList(-1, 3);             // expect: invalid-range-arguments
        CharSequence cs = sb.subSequence(5, 2);               // expect: invalid-range-arguments
        int[] part = Arrays.copyOfRange(values, 4, 2);        // expect: invalid-range-arguments
        int c = Math.clamp(v, 10, 0);                         // expect: invalid-range-arguments
        String s = date.substring(0, -1);                     // expect: invalid-range-arguments
    }

    void fine(String date, List<String> items, Custom custom, int n, int[] values, int v) {
        String year = date.substring(0, 4);
        String rest = date.substring(4);
        String same = date.substring(2, 2);
        List<String> head = items.subList(0, n);
        String x = custom.substring(5, 2);
        int[] part = Arrays.copyOfRange(values, 2, 4);
        int c = Math.clamp(v, 0, 10);
        String y = date.substring(n, 2);
    }

    interface Custom { String substring(int a, int b); }
}
