import java.util.List;

class Closer {
    private boolean open;
    private int counter;

    void reset() {
        counter = 0;
        return;                                                     // expect: unnecessary-return
    }

    void close() {
        if (open) {
            release();
            return;                                                 // expect: unnecessary-return
        } else {
            log();
        }
    }

    void elseIf(int x) {
        if (x > 0) {
            release();
        } else if (x < 0) {
            return;                                                 // expect: unnecessary-return
        }
    }

    void noBraces(boolean b) {
        if (b) return;                                              // expect: unnecessary-return
    }

    Closer() {
        counter = 1;
        return;                                                     // expect: unnecessary-return
    }

    void tryCatch() {
        try {
            release();
            return;                                                 // expect: unnecessary-return
        } catch (RuntimeException e) {
            log();
            return;                                                 // expect: unnecessary-return
        }
    }

    Runnable lambda() {
        return () -> {
            release();
            return;                                                 // expect: unnecessary-return
        };
    }

    void earlyExit() {
        if (open) {
            release();
            return;
        }
        log();
    }

    void inLoop(List<String> items) {
        for (String s : items) {
            if (s.isEmpty()) {
                return;
            }
        }
    }

    void loopBody(List<String> items) {
        while (open) {
            release();
            return;
        }
    }

    void inFinally() {
        try {
            release();
        } finally {
            log();
        }
    }

    void inSwitch(int x) {
        switch (x) {
            case 1:
                release();
                return;
            default:
                log();
        }
    }

    int value() {
        return counter;
    }

    void empty() {
    }

    void release() { }

    void log() { }
}
