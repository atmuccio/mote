// mote-test: test-file
import org.assertj.core.api.AutoCloseableSoftAssertions;
import org.assertj.core.api.BDDSoftAssertions;
import org.assertj.core.api.SoftAssertions;

class UserTest {
    void positives(User user) {
        SoftAssertions softly = new SoftAssertions();                        // expect: soft-assertions-not-verified
        softly.assertThat(user.name()).isEqualTo("Ada");
        softly.assertThat(user.age()).isEqualTo(36);

        var bdd = new BDDSoftAssertions();                                   // expect: soft-assertions-not-verified
        bdd.then(user.name()).isEqualTo("Ada");
    }

    void negatives(User user) {
        SoftAssertions softly = new SoftAssertions();
        softly.assertThat(user.name()).isEqualTo("Ada");
        softly.assertAll();

        SoftAssertions.assertSoftly(s -> s.assertThat(user.age()).isEqualTo(36));

        try (AutoCloseableSoftAssertions auto = new AutoCloseableSoftAssertions()) {
            auto.assertThat(user.age()).isEqualTo(36);
        }

        SoftAssertions passed = new SoftAssertions();
        passed.assertThat(user.age()).isPositive();
        verifyAll(passed);

        SoftAssertions inLambda = new SoftAssertions();
        Runnable r = () -> inLambda.assertAll();
        inLambda.assertThat(user.age()).isPositive();
        r.run();

        Other other = new Other();
        other.assertThat(1);
    }

    void verifyAll(SoftAssertions s) {
        s.assertAll();
    }

    static class Other {
        void assertThat(int x) {}
    }

    record User(String name, int age) {}
}
