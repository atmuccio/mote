import java.time.LocalDate;
import java.time.LocalDateTime;
import java.time.LocalTime;
import java.util.Calendar;
import java.util.GregorianCalendar;

class Schedules {
    void wrong(Calendar cal, int y) {
        LocalDate a = LocalDate.of(2024, 13, 1);                         // expect: date-field-out-of-range
        LocalDate b = LocalDate.of(2024, 2, 30);                         // expect: date-field-out-of-range
        LocalDate c = LocalDate.of(y, 0, 1);                             // expect: date-field-out-of-range
        LocalTime d = LocalTime.of(24, 0);                               // expect: date-field-out-of-range
        LocalTime e = LocalTime.of(10, 60, 0);                           // expect: date-field-out-of-range
        LocalDateTime f = LocalDateTime.of(2024, 4, 31, 12, 0);          // expect: date-field-out-of-range
        Calendar g = new GregorianCalendar(2024, 12, 31);                // expect: date-field-out-of-range
        cal.set(Calendar.MONTH, 12);                                     // expect: date-field-out-of-range
        cal.set(2024, 12, 1);                                            // expect: date-field-out-of-range
    }

    void fine(Calendar cal, int m, Custom custom) {
        LocalDate a = LocalDate.of(2024, 12, 31);
        LocalDate b = LocalDate.of(2024, 2, 29);
        LocalDate c = LocalDate.of(2024, m, 1);
        LocalTime d = LocalTime.of(23, 59, 59, 999_999_999);
        LocalDateTime e = LocalDateTime.of(a, d);
        Calendar f = new GregorianCalendar(2024, Calendar.DECEMBER, 31);
        Calendar g = new GregorianCalendar(2024, 11, 31);
        cal.set(Calendar.MONTH, 11);
        cal.set(Calendar.DAY_OF_MONTH, 0);          // lenient roll-over idiom, not checked
        cal.set(Calendar.HOUR_OF_DAY, 24);
        cal.set(2024, 0, 1);
        custom.of(2024, 13, 1);
        Other.of(13);
    }

    interface Custom { void of(int a, int b, int c); }
    static class Other { static void of(int x) { } }
}
