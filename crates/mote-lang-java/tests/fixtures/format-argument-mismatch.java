import java.io.PrintWriter;
import java.text.MessageFormat;
import java.util.Locale;

class Messages {
    String positives(String user, int count, PrintWriter out, Object[] all) {
        String a = String.format("%s: %s", user);                           // expect: format-argument-mismatch
        String b = String.format("%d items", count, user);                  // expect: format-argument-mismatch
        String c = String.format(Locale.ROOT, "%s and %s", user);           // expect: format-argument-mismatch
        String d = "%s has %d".formatted(user);                             // expect: format-argument-mismatch
        System.out.printf("%s%n", user, count);                             // expect: format-argument-mismatch
        out.format("%s %s %s", user, count);                                // expect: format-argument-mismatch
        String e = MessageFormat.format("{0} of {1}", user);                // expect: format-argument-mismatch
        String f = MessageFormat.format("{0}", user, count);                // expect: format-argument-mismatch
        String g = String.format("%s");                                     // expect: format-argument-mismatch
        String h = String.format("%s %s", oneObject());                     // expect: format-argument-mismatch
        return a + b + c + d + e + f + g + h;
    }

    String negatives(String user, int count, Object[] all, String fmt, Formatter custom, int[] nums) {
        String a = String.format("%s: %d", user, count);
        String b = String.format("100%% of %s%n", user);
        String c = String.format("%2$s %1$s", user, count);                 // explicit indexes
        String d = String.format("%s %s", all);                             // the varargs array itself
        String e = String.format(fmt, user);                                // not a literal
        String f = MessageFormat.format("{0} and {0}", user);
        String g = MessageFormat.format("it''s {0} {1}", user);             // quotes: not modelled
        String h = custom.format("%s %s", user);                            // unknown type
        String i = String.format("%s %s", custom.values());                 // unknown: may be an array
        String j = String.format("%-10s|%5.2f", user, 1.5);
        String k = String.format("%tY-%<tm", new java.util.Date());         // relative index
        String l = "%s".formatted(user);
        String m = String.format("%s", nums);                               // int[] is one argument
        return a + b + c + d + e + f + g + h + i + j + k + l + m;
    }

    Object oneObject() { return null; }                                 // an Object is wrapped, never spread
}
