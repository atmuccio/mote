class ColonSwitchExpression {
    enum Month { FEBRUARY, APRIL, JUNE, SEPTEMBER, NOVEMBER, OTHER }

    int days(Month month) {
        int days = switch (month) {                           // expect: colon-switch-expression
            case FEBRUARY:
                yield 28;
            case APRIL, JUNE, SEPTEMBER, NOVEMBER:
                yield 30;
            default:
                yield 31;
        };
        return days;
    }

    String label(int code) {
        return switch (code) {                                // expect: colon-switch-expression
            case 1: yield "one";
            default: yield "many";
        };
    }

    void call(int code) {
        consume(switch (code) { case 1: yield "a"; default: yield "b"; });   // expect: colon-switch-expression
    }

    void compliant(Month month, int code) {
        int days = switch (month) {
            case FEBRUARY -> 28;
            default -> {
                int base = 30;
                yield base + 1;
            }
        };
        switch (code) {
            case 1:
                consume("one");
                break;
            default:
                consume("many");
        }
        switch (code) {
            case 1 -> consume("one");
            default -> consume("many");
        }
        if (code > 0)
            switch (code) {
                case 2:
                    consume("two");
            }
        label:
        switch (code) {
            case 3:
                break label;
        }
    }

    void consume(String s) {
    }
}
