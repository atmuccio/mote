import java.time.Duration;
import java.util.Date;
import java.util.concurrent.TimeUnit;

class Timeouts {
    private int timeoutSeconds;
    private long deadline;
    private static final long WEEK = 7 * 24 * 3600 * 1000L;
    private static final int MILLIS_PER_SECOND = 1000;
    private static final int SECONDS_PER_HOUR = 60 * 60;
    private static final double SECONDS_PER_DAY = 24 * SECONDS_PER_HOUR;
    private static final long MILLIS_PER_HOUR = SECONDS_PER_HOUR * MILLIS_PER_SECOND;

    long positives(int minutes, int sizeMb, Integer boxed) throws InterruptedException {
        long timeoutMillis = timeoutSeconds * 1000;              // expect: int-multiplication-overflow
        long bytes = sizeMb * 1024 * 1024;                       // expect: int-multiplication-overflow
        double scaled = (minutes * 60 * 1000);                    // expect: int-multiplication-overflow
        deadline = timeoutSeconds * 1000;                         // expect: int-multiplication-overflow
        deadline += minutes * 60000;                              // expect: int-multiplication-overflow
        Thread.sleep(minutes * 60 * 1000);                        // expect: int-multiplication-overflow
        TimeUnit.MILLISECONDS.sleep(timeoutSeconds * 1000);       // expect: int-multiplication-overflow
        Duration d = Duration.ofMillis(minutes * 60 * 1000);      // expect: int-multiplication-overflow
        Date at = new Date(timeoutSeconds * 1000);                // expect: int-multiplication-overflow
        long end = System.currentTimeMillis() + timeoutSeconds * 1000; // expect: int-multiplication-overflow
        long cast = (long) (boxed * 1000);                        // expect: int-multiplication-overflow
        long viaConstant = timeoutSeconds * MILLIS_PER_SECOND;    // expect: int-multiplication-overflow
        return 1000 * minutes;                                    // expect: int-multiplication-overflow
    }

    long negatives(int minutes, byte small, short tiny, long wide, String s) {
        long ok1 = timeoutSeconds * 1000L;          // long constant
        long ok2 = (long) timeoutSeconds * 1000;    // widened first
        long ok3 = minutes * 60;                    // small factor
        int stays = timeoutSeconds * 1000;          // not widened
        var inferred = timeoutSeconds * 1000;       // int variable
        long ok4 = small * 1000;                    // a byte times 1000 always fits
        long ok5 = wide * 1000;                     // already long
        long ok6 = 24 * 3600 * 1000;                // literals only: int-constant-overflow
        long ok7 = minutes * 0x1000;                // hex factors are bit patterns
        long ok8 = unknown() * 1000;                // unknown type
        long ok9 = timeoutSeconds * minutes;        // no constant factor
        long ok10 = s.length() + 1000;              // not a product
        long ok11 = 14 * MILLIS_PER_SECOND;         // constant expression that fits
        long ok12 = minutes * SECONDS_PER_UNKNOWN;  // unknown constant
        boolean ok13 = wide >= -14 * MILLIS_PER_SECOND * 3600; // constant expression that fits
        int[] arr = new int[minutes * 1000];
        foo(minutes * 1000);                        // parameter type not known to be long
        Runnable r = () -> System.out.println(minutes * 1000);
        return ok1 + ok2;
    }

    void foo(int x) {}

    native Object unknown();
}
