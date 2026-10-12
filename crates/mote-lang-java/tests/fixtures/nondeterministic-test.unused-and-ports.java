// mote-test: test-file
import java.util.Random;
import java.util.concurrent.ThreadLocalRandom;
import org.apache.commons.lang3.RandomUtils;
import org.junit.jupiter.api.Test;

class GapDetectorTest {
    // Created but never drawn from: nothing depends on randomness.
    ThreadLocalRandom rand = ThreadLocalRandom.current();
    private final Random unusedToo = new Random();
    private final Random used = new Random();                                // expect: nondeterministic-test

    @Test
    void detectsGaps() {
        Random random = new Random();
        Detector detector = new Detector();
        detector.scan(used.nextInt(100));
    }

    @Test
    void drawsLocally() {
        Random random = new Random();                                        // expect: nondeterministic-test
        int size = random.nextInt(50);
        new Detector().scan(size);
    }

    // A port picked at random only avoids clashes with other servers.
    @Test
    void servesOnRandomPort() {
        int listenPort = RandomUtils.nextInt(10000, 20000);
        Server server = new Server();
        server.setPort(ThreadLocalRandom.current().nextInt(20000, 30000));
        int adminPort = 8000 + new Random().nextInt(1000);
        server.start(listenPort, adminPort);
    }

    private static int createRandomPort() {
        Random random = new Random();
        return random.nextInt((9000 - 8000) + 1) + 8000;
    }

    // Not a port: "report" and "transport" only contain the letters.
    @Test
    void reportsData() {
        int report = RandomUtils.nextInt(1, 10);                             // expect: nondeterministic-test
        int transportSize = ThreadLocalRandom.current().nextInt(64);         // expect: nondeterministic-test
        new Detector().scan(report + transportSize);
    }

    // A test named after ports still draws its data at random.
    @Test
    void parsesPort() {
        int value = new Random().nextInt(65536);                             // expect: nondeterministic-test
        new Detector().scan(value);
    }

    static class Detector {
        void scan(int n) {
        }
    }

    static class Server {
        void setPort(int port) {
        }

        void start(int port, int admin) {
        }
    }
}

// A field of an abstract base class may be drawn from by subclasses in other files.
abstract class RandomDataTestBase {
    protected Random random = new Random();                                   // expect: nondeterministic-test
}
