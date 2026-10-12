import java.util.Arrays;
import java.util.Objects;
import java.util.Random;

class Sharding {
    private final Random random = new Random();

    int wrong(Object key, String name, int[] parts, int buckets) {
        int a = Math.abs(key.hashCode()) % buckets;                 // expect: abs-of-hash-or-random
        int b = Math.abs(Objects.hash(name, key)) % buckets;        // expect: abs-of-hash-or-random
        int c = Math.abs(Arrays.hashCode(parts));                   // expect: abs-of-hash-or-random
        int d = Math.abs(random.nextInt()) % buckets;               // expect: abs-of-hash-or-random
        long e = StrictMath.abs(random.nextLong());                 // expect: abs-of-hash-or-random
        int f = Math.abs(Objects.hashCode(key));                    // expect: abs-of-hash-or-random
        return a + b + c + d + (int) e + f;
    }

    int fine(Object key, int buckets, Scanner in, int delta) {
        int a = Math.abs(key.hashCode() % buckets);
        int b = Math.floorMod(key.hashCode(), buckets);
        int c = Math.abs(random.nextInt(buckets));
        int d = Math.abs(in.nextInt());
        int e = Math.abs(delta);
        int f = (key.hashCode() & 0x7fffffff) % buckets;
        int g = Math.abs(hash(key));
        return a + b + c + d + e + f + g;
    }

    int hash(Object o) { return 1; }

    interface Scanner { int nextInt(); }
}
