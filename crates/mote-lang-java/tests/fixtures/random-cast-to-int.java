import java.util.Random;
import java.util.concurrent.ThreadLocalRandom;

class Dice {
    private final Random random = new Random();

    int wrong(int buckets) {
        int a = (int) Math.random() * buckets;                      // expect: random-cast-to-int
        long b = (long) random.nextDouble();                        // expect: random-cast-to-int
        int c = (int) (ThreadLocalRandom.current().nextFloat());    // expect: random-cast-to-int
        int d = (int) StrictMath.random();                          // expect: random-cast-to-int
        return a + (int) b + c + d;
    }

    int fine(int buckets, Generator gen) {
        int a = (int) (Math.random() * buckets);
        int b = random.nextInt(buckets);
        int c = (int) random.nextGaussian();
        int d = (int) gen.nextDouble();
        double e = (double) Math.random();
        return a + b + c + d + (int) e;
    }

    interface Generator { double nextDouble(); }
}
