import lombok.Builder;
import lombok.Setter;
import lombok.extern.slf4j.Slf4j;

@Builder
class Built {
    private final String name;
    private final int size;
}

class PerField {
    @Setter private int writeOnly;
    private int plain;                                      // expect: unused-private-field
}

@Slf4j
class Logged {
    private int counter;
}
