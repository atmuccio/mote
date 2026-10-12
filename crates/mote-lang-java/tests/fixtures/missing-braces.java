import java.util.List;

class MissingBraces {
    int total;

    void positives(List<Integer> items, boolean ready, int n) {
        if (items.isEmpty())                                  // expect: missing-braces
            return;
        if (ready) total++; else total--;                     // expect: missing-braces, missing-braces
        for (int i = 0; i < n; i++) total += i;               // expect: missing-braces
        for (Integer item : items)                            // expect: missing-braces
            total += item;
        while (n-- > 0) total++;                              // expect: missing-braces
        do total++; while (total < 10);                       // expect: missing-braces
        if (ready) {
            total = 0;
        } else if (n > 3) {
            total = 1;
        } else                                                // expect: missing-braces
            total = 2;
        if (n > 0)                                            // expect: missing-braces
            if (ready) {
                total = 3;
            }
        outer:
        for (Integer a : items)                               // expect: missing-braces
            for (Integer b : items) {
                if (a.equals(b)) {
                    continue outer;
                }
            }
    }

    void compliant(List<Integer> items, boolean ready, int n) {
        if (ready) {
            total++;
        } else if (n > 0) {
            total--;
        } else {
            total = 0;
        }
        for (Integer item : items) {
            total += item;
        }
        while (n-- > 0) {
            total++;
        }
        do {
            total++;
        } while (total < 10);
        // Empty bodies written as `;` are java/empty-statement's business.
        for (int i = 0; i < n; i++);
        while (items.remove(null));
        Runnable r = () -> {
            if (ready) {
                total++;
            }
        };
        switch (n) {
            case 1 -> total++;
            default -> total--;
        }
    }
}
