import java.io.BufferedReader;
import java.util.List;
import java.util.Map;

class Counter {
    private int index;
    private int total;

    void positives(List<String> items, Map<String, Integer> map, int base, int[] arr) {
        process(items.get(index++));                                // expect: assignment-in-operand
        int t = base + (total = compute());                         // expect: assignment-in-operand
        map.put("k", ++index);                                      // expect: assignment-in-operand
        int u = (total += 2) * 3;                                   // expect: assignment-in-operand
        boolean flag = index-- > 0 && ready();                      // expect: assignment-in-operand
        String s = items.get(0) + (index = 3);                      // expect: assignment-in-operand
        Runnable r = () -> process(items.get(index++));             // expect: assignment-in-operand
        if (++total > 10) {                                         // expect: assignment-in-operand
            reset();
        } else if (index-- == 0) {                                  // expect: assignment-in-operand
            reset();
        }
        if (!items.contains(String.valueOf(total++))) {             // expect: assignment-in-operand
            reset();
        }
        if (ready() && base == index++) {                           // expect: assignment-in-operand
            reset();
        }
    }

    void loopsAndIfs(int[] data, int limit) {
        for (int k = limit; --k >= 0;) {                            // loop condition: accepted
            reset();
        }
        do {
            reset();
        } while (--limit > 0);
        if (data[index++] == 0) {                                   // array index: accepted by default
            reset();
        }
        if (ready()) {
            index++;                                                // a statement inside the if
        }
        boolean more = index++ < limit ? ready() : false;           // expect: assignment-in-operand
    }

    void reset() { }

    void negatives(BufferedReader reader, int[] buf, List<String> items) throws Exception {
        index++;
        ++index;
        total = compute();
        index = total = 0;                                          // chained assignment
        int n = index++;                                            // initializer
        total = index++;                                            // right side of an assignment
        for (int i = 0, j = 10; i < j; i++, j--) { }
        String line;
        while ((line = reader.readLine()) != null) { }              // java/assignment-in-condition
        while (index-- > 0) { }                                     // loop condition
        if ((total = compute()) > 3) { }
        buf[index++] = 1;                                           // array index: accepted by default
        int v = buf[index++];
        Runnable r = () -> index++;
        items.forEach(x -> total += x.length());
    }

    int returned() {
        return index++;                                             // a return value
    }

    void process(String s) { }

    int compute() { return 1; }

    boolean ready() { return true; }
}
