import javax.persistence.Entity;
import lombok.Data;

class Settings {
    private final int maxRetries = 3;                               // expect: final-field-could-be-static
    private final String prefix = "job-";                           // expect: final-field-could-be-static
    final long timeout = 60 * 1000L;                                // expect: final-field-could-be-static
    private final boolean enabled = !false;                         // expect: final-field-could-be-static
    private final char sep = ',', quote = '"';                      // expect: final-field-could-be-static, final-field-could-be-static
    private final long serialVersionUID = 1L;                       // expect: final-field-could-be-static
    private final double ratio = -0.5;                              // expect: final-field-could-be-static
    private final String joined = "a" + "b";                        // expect: final-field-could-be-static

    private static final int ALREADY = 1;
    private final int computed = compute();
    private final Integer boxed = 5;                                // expect: final-field-could-be-static
    private final String nothing = null;
    private final int noValue;
    private final int fromConstant = ALREADY;                       // names are not considered
    private final int[] array = {1, 2};
    private int notFinal = 1;
    @Deprecated
    private final int annotated = 2;

    Settings() {
        noValue = 1;
    }

    int compute() {
        return 1;
    }
}

interface Api {
    int VALUE = 1;
}

@Entity
class Row {
    private final String type = "row";                              // persisted by reflection
}

@Data
class Payload {
    private final String kind = "payload";                          // Lombok generates a getter
}

enum Mode {
    ON;

    private final int weight = 10;                                  // expect: final-field-could-be-static
}

record Point(int x, int y) {
}
