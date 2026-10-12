import lombok.Getter;
import lombok.extern.slf4j.Slf4j;

@Slf4j
class MailSender {
    private final int port = 25;                                    // expect: final-field-could-be-static
    private final Long timeout = 30_000L;                           // expect: final-field-could-be-static
    private final Boolean secure = Boolean.TRUE;                    // not a literal
    private final Integer none = null;                              // null: not a constant value
    private final java.lang.Double ratio = 0.5;                     // expect: final-field-could-be-static
    private final Number number = 1;                                // not a boxed primitive type

    void send() {
        log.info("port {}", port);
    }
}

@Getter
class Settings {
    private final int port = 25;                                    // a generated getter reads it
}

class Custom {
    private final Integer value = new Integer();                    // `Integer` of this file, not java.lang

    static class Integer {
    }
}
