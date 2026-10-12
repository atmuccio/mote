// mote-test: params report_unknown_message_patterns=true
import java.text.MessageFormat;

class Labels {
    private static final MessageFormat FROM_BUNDLE = new MessageFormat(Bundle.get("label"));  // expect: static-date-format
    private static MessageFormat later;                                                       // expect: static-date-format
    private static final MessageFormat GREETING = new MessageFormat("Hello {0}");
}
