import java.io.Serial;
import java.io.Serializable;
import java.util.List;

class Session {
    // The serial marker is an annotation: annotated members are left alone.
    private record Snapshot(String name, List<String> roles) implements Serializable {
        @Serial
        private static final long serialVersionUID = 1L;
    }

    private record Fully(String name) implements Serializable {
        @java.io.Serial
        private static final long serialVersionUID = 2L;
    }

    private record Plain(String name) implements Serializable {
        private static final long serialVersionUID = 3L;               // expect: redundant-record-member
    }
}
