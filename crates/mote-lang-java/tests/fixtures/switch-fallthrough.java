class Fallthrough {
    int warnings, errors;

    void positives(int level, boolean flag) {
        switch (level) {
            case 1: // expect: switch-fallthrough
                warnings++;
            case 2:
                errors++;
                break;
            case 3: // expect: switch-fallthrough
                if (flag) {
                    break;
                }
            case 4: // expect: switch-fallthrough
                if (flag) {
                    return;
                } else {
                    warnings++;
                }
            case 5: // expect: switch-fallthrough
                {
                    warnings++;
                }
            case 6:
            case 7: // expect: switch-fallthrough
                errors++;
            default:
                warnings++;
        }
        int v = switch (level) {
            case 1: // expect: switch-fallthrough
                warnings++;
            case 2:
                yield 3;
            default:
                yield 4;
        };
    }

    int negatives(int level, boolean flag) {
        switch (level) {
            case 1:
            case 2:
                warnings++;
                break;
            case 3:
                warnings++;
                // falls through
            case 4:
                errors++;
                /* FALLTHROUGH */
            case 5:
                errors++;
                // fall-through intended
            case 6:
                return 1;
            case 7:
                throw new IllegalStateException();
            case 8:
                if (flag) {
                    return 2;
                } else {
                    break;
                }
            case 9:
                try {
                    return 3;
                } finally {
                    warnings++;
                }
            case 10:
                while (true) {
                    if (flag) break;
                }
            case 11:
                fail("unexpected");
            case 12:
                System.exit(1);
            case 13:
                synchronized (this) {
                    break;
                }
            case 14:
                for (int i = 0; i < 3; i++) {
                    continue;
                }
            default:
                warnings++;
        }
        for (int i = 0; i < 3; i++) {
            switch (i) {
                case 0:
                    continue;
                case 1:
                    errors++;
                    break;
            }
        }
        switch (level) {
            case 1 -> warnings++;
            case 2 -> errors++;
            default -> { }
        }
        return 0;
    }

    void emptyDefault(int state) {
        switch (state) {
            case 0:
                break;
            case 1:
                stop();
            default:
        }
        switch (state) {
            case 0:
                stop();
            case 1:
                // nothing to do
            default:
                // ignored
        }
        switch (state) {
            case 0:                                             // expect: switch-fallthrough
                stop();
            case 1:
            default:
                stop();
        }
    }

    void stop() { }

    void fail(String m) { throw new AssertionError(m); }
}
