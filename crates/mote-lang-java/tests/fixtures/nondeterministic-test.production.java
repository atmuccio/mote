import java.util.Random;

class Jitter {
    private final Random random = new Random();

    long delay() {
        return (long) (Math.random() * 100);
    }
}
