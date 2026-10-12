import java.text.MessageFormat;

class Labels {
    private static final MessageFormat PRICE = new MessageFormat("{0} costs {1,number,#.##}"); // expect: static-date-format
    static final MessageFormat DATED = new MessageFormat("On {0,date,short}");                // expect: static-date-format
    static MessageFormat mutable = new MessageFormat("{0} of {1,choice,0#none|1#one}");       // expect: static-date-format

    // A pattern read at run time is unknown (reported with report_unknown_message_patterns).
    private static final MessageFormat FROM_BUNDLE = new MessageFormat(Bundle.get("label"));
    // Plain placeholders: format() creates any number or date format afresh.
    private static final MessageFormat GREETING = new MessageFormat("Hello {0}, you have {1} items");
    private final MessageFormat perInstance = new MessageFormat("{0,number}");
    private static final ThreadLocal<MessageFormat> LOCAL =
            ThreadLocal.withInitial(() -> new MessageFormat("{0,number}"));

    String price(Object item, double p) {
        return PRICE.format(new Object[] {item, p}) + GREETING.format(new Object[] {item, 1});
    }
}
