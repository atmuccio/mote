// mote-test: test-file
import static org.assertj.core.api.Assertions.assertThat;

import java.util.List;
import org.assertj.core.api.Assertions;
import org.assertj.core.api.SoftAssertions;

class OrderTest {
    void positives(int total, List<String> items, String name) {
        assertThat(total).isEqualTo(10).as("order total");                  // expect: assertion-description-after-check
        assertThat(items).hasSize(2).describedAs("items");                   // expect: assertion-description-after-check
        Assertions.assertThat(name).startsWith("A").withFailMessage("name"); // expect: assertion-description-after-check
        assertThat(items).isNotEmpty().contains("a").as("x").as("y");        // expect: assertion-description-after-check
        SoftAssertions softly = new SoftAssertions();
        softly.assertThat(total).isPositive().as("total");                   // expect: assertion-description-after-check
        softly.assertAll();
    }

    void negatives(int total, List<String> items, Helper helper) {
        assertThat(total).as("order total").isEqualTo(10);
        assertThat(total).isNotNull().as("then").isEqualTo(10);
        assertThat(items).first().as("first");
        helper.check(total).isValid().as("custom");
        Object o = assertThat(total).isEqualTo(10).as("kept");
    }

    interface Helper {
        Helper check(int t);
        Helper isValid();
        Helper as(String d);
    }
}
