import java.util.List;

class Loops {
    void positives(String[] parts, int[][] grid, int n, List<String> queue) {
        for (int i = 0; i < parts.length; i++) {
            process(parts[i]);
            i += 2;                                                   // expect: for-loop-counter-misuse
        }
        for (int i = 0; i < n; i++) {
            if (parts[i].startsWith("#")) {
                i = parts[i].length();                                // expect: for-loop-counter-misuse
            }
        }
        for (int row = 0; row < grid.length; row++) {
            for (int col = 0; col < grid[row].length; row++) {        // expect: for-loop-counter-misuse
                grid[row][col] = 0;
            }
        }
        for (int i = n; i >= 0; i--) {
            process(parts[i]);
            i -= 1;                                                   // expect: for-loop-counter-misuse
        }
        for (int i = 0; i < queue.size(); i++) {
            if (queue.get(i).isEmpty()) {
                found = true;
                i = queue.size();                                     // expect: for-loop-counter-misuse
            }
        }
        for (int i = 0; i < n; count++) {                             // expect: for-loop-counter-misuse
            process(parts[i]);
        }
    }

    void negatives(String[] parts, List<String> items, int n, int[] ftab, int[] order) {
        // ++ / -- to consume or step back: accepted.
        for (int i = 0; i < parts.length; i++) {
            if (parts[i].equals("-f")) {
                file = parts[++i];
            }
        }
        for (int i = 0; i < items.size(); i++) {
            if (items.get(i).isEmpty()) {
                items.remove(i);
                i--;
            }
        }
        // A longer skip under a condition: accepted.
        for (int i = 0; i < parts.length; i++) {
            if (parts[i].equals("%")) {
                decode(parts[i + 1], parts[i + 2]);
                i += 2;
            }
        }
        for (int i = 0; i < parts.length; i++) {
            if (parts[i].isEmpty()) {
                continue;
            }
            decode(parts[i + 1], parts[i + 2]);
            i += 2;
        }
        for (int i = 0; i < parts.length; i++) {
            switch (parts[i]) {
                case "--pair":
                    i += 2;
                    break;
                default:
                    process(parts[i]);
            }
        }
        // Tested with != (another rule's business).
        for (int i = 0; i != n; i++) {
            i += 1;
        }
        // No update: the body advances the counter, visibly.
        for (int i = 0; i < parts.length; ) {
            i += width(parts[i]);
        }
        // Counter declared outside the header.
        int j = 0;
        for (; j < n; j++) {
            j += 2;
        }
        // The update changes a variable the condition tests.
        for (int i = 0, k = n; i < k; k--) {
            process(parts[i]);
        }
        // The counter is used as an index in the condition.
        for (int r = order[0]; ftab[r + 1] > n; r = order[r]) {
            process(parts[r]);
        }
        // Written in a lambda.
        for (int i = 0; i < n; i++) {
            Runnable r = () -> {
                int i2 = 0;
                i2 += 1;
            };
        }
        // Other variables.
        for (int i = 0; i < n; i++) {
            total += i;
        }
        // Iterator loops.
        for (java.util.Iterator<String> it = items.iterator(); it.hasNext(); count++) {
            process(it.next());
        }
    }

    private String file;
    private int total;
    private int count;
    private boolean found;

    int width(String s) { return 1; }

    void decode(String a, String b) {}

    void process(String s) {}
}
