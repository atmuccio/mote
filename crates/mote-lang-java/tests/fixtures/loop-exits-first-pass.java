import java.util.Iterator;
import java.util.List;

class LoopExits {

    String firstOpen(List<String> orders) {
        for (String order : orders) {
            if (order.isEmpty()) {
                log(order);
            }
            return order;                                    // expect: loop-exits-first-pass
        }
        return null;
    }

    void closeOne(List<String> orders) {
        for (String order : orders) {
            close(order);
            break;                                           // expect: loop-exits-first-pass
        }
    }

    void whileOnce(Iterator<String> it) {
        while (it.hasNext()) {
            String s = it.next();
            if (s == null) {
                break;
            }
            throw new IllegalStateException(s);              // expect: loop-exits-first-pass
        }
    }

    int indexed(int[] values) {
        for (int i = 0; i < values.length; i++)
            return values[i];                                // expect: loop-exits-first-pass
        return -1;
    }

    void labeledOuter(int[][] grid) {
        outer:
        for (int[] row : grid) {
            for (int cell : row) {
                if (cell == 0) {
                    continue;                                // targets the inner loop only
                }
                log("x");
            }
            break outer;                                     // expect: loop-exits-first-pass
        }
    }

    void doOnce(Iterator<String> it) {
        do {
            log(it.next());
            break;                                           // expect: loop-exits-first-pass
        } while (it.hasNext());
    }

    void innerContinueDoesNotCount(List<List<String>> groups) {
        for (List<String> g : groups) {
            for (String s : g) {
                if (s.isEmpty()) continue;
                log(s);
            }
            return;                                          // expect: loop-exits-first-pass
        }
    }

    // ---- compliant ------------------------------------------------------------------------

    String retry(Iterator<String> it) {
        while (it.hasNext()) {
            String s = it.next();
            if (s.isEmpty()) {
                continue;
            }
            return s;
        }
        return null;
    }

    void labeledContinue(List<List<String>> groups) {
        outer:
        for (List<String> g : groups) {
            for (String s : g) {
                if (s.isEmpty()) {
                    continue outer;
                }
            }
            return;
        }
    }

    void switchContinue(List<String> items) {
        for (String s : items) {
            switch (s) {
                case "a":
                    continue;
                default:
                    log(s);
            }
            break;
        }
    }

    void doWhileFalse(boolean a) {
        do {
            if (a) break;
            log("b");
            break;
        } while (false);
    }

    void conditionalBreak(List<String> items) {
        for (String s : items) {
            if (s.isEmpty()) {
                break;
            }
        }
    }

    void breakInSwitch(List<String> items) {
        for (String s : items) {
            switch (s) {
                case "a":
                    log(s);
                    break;
                default:
                    break;
            }
        }
    }

    void lambdaReturn(List<Runnable> tasks, List<String> items) {
        for (String s : items) {
            tasks.add(() -> {
                log(s);
                return;
            });
        }
    }

    void anonymousClass(List<Object> out, List<String> items) {
        for (String s : items) {
            out.add(new Object() {
                @Override
                public String toString() {
                    for (int i = 0; i < 3; i++) {
                        if (i == s.length()) continue;
                        return s;
                    }
                    return "";
                }
            });
        }
    }

    void tryAtEnd(Iterator<String> it) {
        while (true) {
            try {
                log(it.next());
                return;
            } catch (RuntimeException e) {
                log("retry");
            }
        }
    }

    void close(String s) {}

    void log(String s) {}
}
