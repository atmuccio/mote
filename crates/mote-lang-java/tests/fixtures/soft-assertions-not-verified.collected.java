// mote-test: test-file
import static org.assertj.core.api.Assertions.assertThat;

import java.util.List;
import org.assertj.core.api.SoftAssertions;
import org.junit.jupiter.api.Test;

class CollectedTest {
    // The test reads the collected failures and asserts on them: that is verification.
    @Test
    void readsCollectedErrors() {
        SoftAssertions softly = new SoftAssertions();
        softly.assertThat("abc").contains("z");
        List<AssertionError> errors = softly.assertionErrorsCollected();
        assertThat(errors).hasSize(1);
    }

    @Test
    void comparesCollectedErrors() {
        SoftAssertions softly = new SoftAssertions();
        softly.assertThat(1).isEqualTo(2);
        assertThat(softly.errorsCollected()).isNotEmpty();
    }

    @Test
    void forgotten() {
        SoftAssertions softly = new SoftAssertions();                   // expect: soft-assertions-not-verified
        softly.assertThat(1).isEqualTo(2);
    }
}
