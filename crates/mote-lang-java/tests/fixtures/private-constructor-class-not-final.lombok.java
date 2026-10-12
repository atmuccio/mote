import lombok.Builder;
import lombok.Data;
import lombok.Getter;
import lombok.RequiredArgsConstructor;
import lombok.experimental.UtilityClass;
import lombok.extern.slf4j.Slf4j;

@Slf4j
class Files {                                                       // expect: private-constructor-class-not-final
    private Files() {
    }

    static void touch(String path) {
        log.info(path);
    }
}

@Getter
class Event {                                                       // expect: private-constructor-class-not-final
    private final String id;

    private Event(String id) {
        this.id = id;
    }

    static Event of(String id) {
        return new Event(id);
    }
}

@lombok.extern.log4j.Log4j2
class Qualified {                                                   // expect: private-constructor-class-not-final
    private Qualified() {
    }
}

@Data
class Row {                                                         // @Data may generate a constructor
    private Row() {
    }
}

@Builder
class Order {                                                       // the builder calls a generated constructor
    private final String id;

    private Order(String id) {
        this.id = id;
    }
}

@RequiredArgsConstructor
class Service {
    private Service(int x) {
    }
}

@UtilityClass
class Strings {
    private Strings(int x) {
    }
}
