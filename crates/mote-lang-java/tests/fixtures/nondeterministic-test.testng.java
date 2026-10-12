// mote-test: test-file
import org.testng.annotations.Test;

class RetriedTest {
    @Test(retryAnalyzer = RetryThreeTimes.class)                          // expect: nondeterministic-test
    void flaky() {
    }

    @Test(groups = "slow")
    void stable() {
    }
}
