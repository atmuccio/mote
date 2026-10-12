// mote-test: params java_release=21
import java.math.BigDecimal;
import java.util.function.Function;

class SwitchArms {
    enum Mode { UPPER, LOWER, KEEP }

    String returned(String s, Mode mode) {
        return switch (mode) {
            case UPPER -> s.toUpperCase();
            case LOWER -> s.toLowerCase();
            case KEEP -> s;
        };
    }

    String assigned(String s, Mode mode) {
        String r = switch (mode) {
            case UPPER -> s.trim();
            default -> s.strip();
        };
        return r;
    }

    void argument(String s, Mode mode, char c) {
        use(switch (mode) {
            case UPPER -> s.substring(1);
            default -> String.valueOf(c);
        });
    }

    Function<String, String> lambda(Mode mode) {
        return s -> switch (mode) {
            case UPPER -> s.toUpperCase();
            default -> s.concat("!");
        };
    }

    BigDecimal nested(BigDecimal a, Mode mode, boolean flag) {
        return switch (mode) {
            case UPPER -> switch (flag ? 1 : 0) {
                case 1 -> a.negate();
                default -> a.abs();
            };
            default -> a;
        };
    }

    void statementSwitch(String s, Mode mode, BigDecimal a) {
        switch (mode) {
            case UPPER -> s.toUpperCase();                          // expect: ignored-return-value
            case LOWER -> {
                s.toLowerCase();                                    // expect: ignored-return-value
            }
            default -> a.negate();                                  // expect: ignored-return-value
        }
        if (s.isEmpty())
            switch (mode) {
                case KEEP -> s.trim();                              // expect: ignored-return-value
                default -> { }
            }
    }

    String blockArm(String s, Mode mode) {
        return switch (mode) {
            case UPPER -> {
                s.trim();                                           // expect: ignored-return-value
                yield s;
            }
            default -> s;
        };
    }

    void use(String s) { }
}
