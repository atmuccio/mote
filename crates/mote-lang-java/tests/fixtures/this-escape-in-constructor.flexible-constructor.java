import java.util.Objects;

// Java 25 flexible constructor bodies: validation may come before `super()`; what the
// constructor does with `this` afterwards is judged as usual.
class Sensor {
    Sensor(Hub hub, String id) {
        if (id.isBlank()) {
            throw new IllegalArgumentException("id");
        }
        super();
        hub.register(this);                                          // expect: this-escape-in-constructor
        Objects.requireNonNull(hub);
    }

    interface Hub {
        void register(Object o);
    }
}

final class Probe {
    Probe(Sensor.Hub hub, String id) {
        Objects.requireNonNull(id);
        super();
        hub.register(this);
    }
}
