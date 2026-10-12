import java.util.List;

class Unreachable {
    int positives(int x, List<String> items) {
        if (x > 0) {
            return 1;
            log("never"); // expect: unreachable-code
        }
        for (String s : items) {
            if (s.isEmpty()) {
                continue;
                log(s); // expect: unreachable-code
            }
            break;
            log(s); // expect: unreachable-code
        }
        switch (x) {
            case 1:
                log("one");
                break;
                log("after break"); // expect: unreachable-code
            case 2:
                throw new IllegalStateException();
            default:
                log("other");
        }
        return 0;
        // a comment after the return is fine
    }

    Unreachable() {
        throw new UnsupportedOperationException();
        log("x"); // expect: unreachable-code
    }

    int twice(int x) {
        return x;
        log("a"); // expect: unreachable-code
        return x + 1;
        log("b");                  // only the first unreachable statement is reported
    }

    int negatives(int x) {
        if (x > 0) {
            return 1;
        } else {
            log("else");
        }
        Runnable r = () -> {
            log("in lambda");
            return;
        };
        switch (x) {
            case 1:
                log("one");
                break;
            case 2:
                log("two");
                // fall through
            default:
                log("other");
        }
        int y = switch (x) {
            case 1 -> {
                log("one");
                yield 1;
            }
            default -> 2;
        };
        return y;
    }

    void emptyAfterReturn() {
        return;;
    }

    void log(String s) {}
}
