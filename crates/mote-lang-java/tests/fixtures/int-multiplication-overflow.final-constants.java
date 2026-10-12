import java.util.concurrent.TimeUnit;

class Batching {
    final int chunkBytes = 64 * 1024;
    final long totalMessages = 5000;
    private final int retries;
    private int window = 16;

    Batching(int retries) {
        this.retries = retries;
    }

    long limits(int chunks) {
        // Every factor is a constant: the product is known and fits easily.
        final int count = 10;
        final int kilobytes = 4;
        long span = 20000 + (count * 1000);
        long needed = count * kilobytes * 1024;
        long budget = (totalMessages * chunkBytes) + (1000 * chunkBytes);
        // A variable factor is still reported, even next to a final constant.
        long sized = chunks * kilobytes * 1024;                       // expect: int-multiplication-overflow
        // A final field without initializer, or a non-final local, can hold anything.
        long tries = retries * 60 * 1000;                             // expect: int-multiplication-overflow
        int pause = 30;
        long waited = pause * 1000;                                   // expect: int-multiplication-overflow
        long win = window * 1000;                                     // expect: int-multiplication-overflow
        return span + needed + budget + sized + tries + waited + win;
    }

    void sleepy() throws InterruptedException {
        final int seconds = 5;
        TimeUnit.MILLISECONDS.sleep(seconds * 1000);
        Thread.sleep(seconds * 60 * 1000);
    }
}
