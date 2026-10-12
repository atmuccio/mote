import java.util.Objects;
import java.util.Random;

class Buckets {
    private final Random random = new Random();

    int positives(Object key, String a, String b) {
        int h = key.hashCode();
        int bucket = (h < 0 ? -h : h) % 16;                     // expect: abs-of-hash-or-random
        int n = random.nextInt();
        if (n < 0) {
            n = -n;                                             // expect: abs-of-hash-or-random
        }
        int m = Objects.hash(a, b);
        int k = m >= 0 ? m : -m;                                // expect: abs-of-hash-or-random
        int d = key.hashCode() < 0 ? -key.hashCode() : key.hashCode();   // expect: abs-of-hash-or-random
        return bucket + n + k + d;
    }

    int negatives(Object key, int value) {
        int h = key.hashCode();
        int flipped = -h;                                       // not an abs
        int v = value < 0 ? -value : value;                     // not a hash
        int r = random.nextInt(10);
        int s = r < 0 ? -r : r;                                 // bounded
        int t = h < 0 ? h : -h;                                 // makes it non-positive: fine
        int u = (h & 0x7fffffff) % 16;
        return flipped + v + s + t + u;
    }
}

class Reduced {
    int bucket(Object key, int buckets) {
        int h = key.hashCode();
        h = h % buckets;                                        // reduced: -h is safe now
        int idx = h < 0 ? -h : h;
        int g = key.hashCode();
        g++;
        if (g < 0) {
            g = -g;
        }
        return idx + g;
    }
}
