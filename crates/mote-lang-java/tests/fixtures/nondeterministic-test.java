// mote-test: test-file
import java.security.SecureRandom;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Random;
import java.util.UUID;
import java.util.concurrent.ThreadLocalRandom;
import org.apache.commons.lang3.RandomStringUtils;
import org.junit.jupiter.api.Test;
import org.junitpioneer.jupiter.RetryingTest;

class SorterTest {
    @Test
    void sortsAnyInput() {
        int[] data = new Random().ints(100).toArray();                    // expect: nondeterministic-test
        double d = Math.random();                                         // expect: nondeterministic-test
        int n = ThreadLocalRandom.current().nextInt(10);                  // expect: nondeterministic-test
        List<Integer> list = new ArrayList<>();
        Collections.shuffle(list);                                        // expect: nondeterministic-test
        String name = RandomStringUtils.randomAlphabetic(8);              // expect: nondeterministic-test
    }

    @RetryingTest(3)                                                      // expect: nondeterministic-test
    void flaky() {
    }

    @Test
    void deterministic() {
        Random seeded = new Random(42);
        Collections.shuffle(new ArrayList<>(), seeded);
        byte[] key = new byte[16];
        new SecureRandom().nextBytes(key);
        String id = UUID.randomUUID().toString();
        String table = "tmp_" + new Random().nextInt(1000);                // unique name
        String queue = "q" + Math.random();                                // unique name
        String file = String.valueOf(ThreadLocalRandom.current().nextLong()); // unique name
        long seed = new Random().nextLong();                               // drawing a seed
        Random replay = new Random(seed);
        Random later = new Random();                                       // seeded next
        later.setSeed(7);
    }
}
