// mote-test: test-file
import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatCode;
import static org.assertj.core.api.Assertions.assertThatExceptionOfType;

import org.assertj.core.api.Assertions;
import org.assertj.core.api.SoftAssertions;
import org.junit.jupiter.api.Test;

class OrderTest {
    @Test
    void wrong(Order order, SoftAssertions softly) {
        assertThat(order.isValid());                                         // expect: incomplete-fluent-assertion
        assertThat(order.total()).as("total");                               // expect: incomplete-fluent-assertion
        Assertions.assertThat(order.id() > 0);                               // expect: incomplete-fluent-assertion
        assertThatCode(() -> order.submit());                                // expect: incomplete-fluent-assertion
        assertThatExceptionOfType(IllegalStateException.class);              // expect: incomplete-fluent-assertion
        softly.assertThat(order.lines());                                    // expect: incomplete-fluent-assertion
        assertThat(order.total())                                            // expect: incomplete-fluent-assertion
                .describedAs("the total")
                .withFailMessage("bad");
    }

    @Test
    void fine(Order order, SoftAssertions softly) {
        assertThat(order.isValid()).isTrue();
        assertThat(order.total()).as("total").isEqualTo(3);
        assertThatCode(() -> order.submit()).doesNotThrowAnyException();
        assertThatExceptionOfType(IllegalStateException.class).isThrownBy(() -> order.submit());
        softly.assertThat(order.lines()).isEmpty();
        Object a = assertThat(order.total());
        org.hamcrest.MatcherAssert.assertThat(order.total(), org.hamcrest.Matchers.is(3));
        order.as("x");
    }

    interface Order {
        boolean isValid();
        int total();
        int id();
        void submit();
        java.util.List<String> lines();
        Order as(String s);
    }
}

class CustomHelperTest {
    static void assertThat(boolean condition) {
        if (!condition) throw new AssertionError();
    }

    @Test
    void usesHelper() {
        assertThat(1 + 1 == 2);
    }
}

class UnknownOriginTest extends SomeBaseTest {
    @Test
    void inherited() {
        verifyThat(3);
    }
}
