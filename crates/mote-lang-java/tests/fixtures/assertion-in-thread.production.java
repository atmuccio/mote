import static org.junit.jupiter.api.Assertions.assertEquals;

// Not a test file: the rule stays silent.
class Worker {
    void start() {
        new Thread(() -> assertEquals(1, compute())).start();
    }

    int compute() {
        return 1;
    }
}
