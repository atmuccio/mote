import java.text.SimpleDateFormat;
import java.time.format.DateTimeFormatter;

class Weeks {
    private static final DateTimeFormatter WEEK = DateTimeFormatter.ofPattern("yyyy-'W'ww");   // expect: week-year-pattern

    void positives(SimpleDateFormat sdf) {
        SimpleDateFormat a = new SimpleDateFormat("yyyy/ww");                    // expect: week-year-pattern
        DateTimeFormatter b = DateTimeFormatter.ofPattern("uuuu 'week' w");      // expect: week-year-pattern
        sdf.applyPattern("ww.yy");                                               // expect: week-year-pattern
    }

    void negatives(SimpleDateFormat sdf) {
        DateTimeFormatter a = DateTimeFormatter.ofPattern("YYYY-'W'ww-e");       // ISO week date
        SimpleDateFormat b = new SimpleDateFormat("yyyy-MM-dd (ww)");            // week shown next to a full date
        SimpleDateFormat c = new SimpleDateFormat("yyyy-DDD ww");                // day of year pins the year
        SimpleDateFormat d = new SimpleDateFormat("u 'of week' w");              // `u` is a day number here
        DateTimeFormatter e = DateTimeFormatter.ofPattern("'yyyy' ww");          // quoted
        SimpleDateFormat f = new SimpleDateFormat("W 'of' MMMM yyyy");           // week in month
    }
}
