import java.text.DateFormat;
import java.text.DecimalFormat;
import java.text.NumberFormat;
import java.text.SimpleDateFormat;
import java.time.format.DateTimeFormatter;
import java.util.Calendar;
import java.util.Date;
import java.util.GregorianCalendar;

class Formats {
    private static final SimpleDateFormat DAY = new SimpleDateFormat("yyyy-MM-dd");   // expect: static-date-format
    public static final DateFormat TIME = DateFormat.getTimeInstance();              // expect: static-date-format
    static NumberFormat money;                                                       // expect: static-date-format
    protected static final DecimalFormat PCT = new DecimalFormat("0.0%");            // expect: static-date-format
    static final Calendar CAL = Calendar.getInstance(), OTHER = new GregorianCalendar(); // expect: static-date-format, static-date-format
    private static final java.text.SimpleDateFormat QUALIFIED = new java.text.SimpleDateFormat("HH"); // expect: static-date-format

    String day(Date d) {
        return DAY.format(d);
    }

    String qualified(Date d) {
        String s;
        synchronized (Formats.class) {
            s = QUALIFIED.format(d);
        }
        return QUALIFIED.toPattern() + s;
    }

    interface Constants {
        DateFormat ISO = new SimpleDateFormat("yyyy");                               // expect: static-date-format
    }

    // negatives
    private final SimpleDateFormat perInstance = new SimpleDateFormat("yyyy");
    private static final DateTimeFormatter SAFE = DateTimeFormatter.ISO_DATE;
    private static final ThreadLocal<SimpleDateFormat> TL =
            ThreadLocal.withInitial(() -> new SimpleDateFormat("yyyy-MM-dd"));
    private static final ThreadLocal<DateFormat> TL2 = new ThreadLocal<DateFormat>() {
        @Override protected DateFormat initialValue() { return new SimpleDateFormat("HH:mm"); }
    };
    private static final FastDateFormat THIRD_PARTY = FastDateFormat.getInstance("yyyy");
    private static final String PATTERN = "yyyy-MM-dd";
    private static final SimpleDateFormat[] ARRAY = new SimpleDateFormat[2];

    private static final SimpleDateFormat GUARDED = new SimpleDateFormat("yyyy");
    private static final SimpleDateFormat CLONED = new SimpleDateFormat("MM");

    static synchronized String guarded(Date d) {
        return GUARDED.format(d);
    }

    String guardedBlock(Date d) {
        synchronized (GUARDED) {
            return GUARDED.format(d);
        }
    }

    String cloned(Date d) {
        return ((DateFormat) CLONED.clone()).format(d);
    }

    void local() {
        SimpleDateFormat f = new SimpleDateFormat("yyyy");
        f.format(new Date());
    }
}
