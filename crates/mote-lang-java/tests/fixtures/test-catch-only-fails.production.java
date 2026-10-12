import static org.junit.Assert.fail;

// Not a test file: the rule stays silent.
class TestSupport {
    void check() {
        try {
            run();
        } catch (Exception e) {
            fail(e.getMessage());
        }
    }

    void run() throws Exception {}
}
