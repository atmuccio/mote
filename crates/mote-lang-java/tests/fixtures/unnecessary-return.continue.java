import java.util.List;

class Loops {
    void positives(List<String> items, int n) {
        for (String item : items) {
            if (item.isEmpty()) {
                evict(item);
                continue;                                   // expect: unnecessary-return
            }
        }
        for (int i = 0; i < n; i++) {
            log(i);
            continue;                                       // expect: unnecessary-return
        }
        while (n-- > 0) {
            if (n % 2 == 0) {
                log(n);
            } else {
                continue;                                   // expect: unnecessary-return
            }
        }
        do {
            try {
                log(n);
                continue;                                   // expect: unnecessary-return
            } catch (RuntimeException e) {
                log(-1);
            }
        } while (n++ < 3);
    }

    void negatives(List<String> items, int n) {
        for (String item : items) {
            if (item.isEmpty()) {
                continue;                                   // skips the log
            }
            log(item.length());
        }
        outer:
        for (int i = 0; i < n; i++) {
            for (int j = 0; j < n; j++) {
                if (j == i) {
                    continue outer;                         // continues the outer loop
                }
            }
        }
        for (int i = 0; i < n; i++) {
            switch (i) {
                case 1:
                    log(i);
                    continue;                               // removing it would fall through
                default:
                    log(0);
            }
        }
        Runnable r = () -> {
            for (String item : items) {
                continue;                                   // expect: unnecessary-return
            }
        };
    }

    void evict(String s) { }

    void log(int i) { }
}
