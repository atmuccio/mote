import java.text.SimpleDateFormat;
import java.time.format.DateTimeFormatter;
import java.util.Locale;

class Dates {
    private static final DateTimeFormatter ISO = DateTimeFormatter.ofPattern("YYYY-MM-dd");   // expect: week-year-pattern

    void positives(SimpleDateFormat sdf) {
        SimpleDateFormat a = new SimpleDateFormat("dd/MM/YYYY");                 // expect: week-year-pattern
        SimpleDateFormat b = new java.text.SimpleDateFormat("YYYYMMdd", Locale.ROOT);  // expect: week-year-pattern
        DateTimeFormatter c = DateTimeFormatter.ofPattern("YY-MM", Locale.US);   // expect: week-year-pattern
        sdf.applyPattern("'Date:' YYYY.MM.dd");                                  // expect: week-year-pattern
    }

    void negatives(SimpleDateFormat sdf, Formats f) {
        SimpleDateFormat a = new SimpleDateFormat("yyyy-MM-dd");
        DateTimeFormatter b = DateTimeFormatter.ofPattern("YYYY-'W'ww");        // ISO week date
        DateTimeFormatter c = DateTimeFormatter.ofPattern("uuuu-MM-dd");
        DateTimeFormatter d = DateTimeFormatter.ofPattern("'Year' yyyy-MM-dd");
        DateTimeFormatter e = DateTimeFormatter.ofPattern("'YYYY' yyyy-MM-dd");  // quoted text
        SimpleDateFormat g = new SimpleDateFormat("YYYY");                       // no month or day
        f.ofPattern("YYYY-MM-dd");                                               // unknown type
    }
}
