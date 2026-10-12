import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

class Inventory {
    private final List<Car> cars = new ArrayList<>();
    private final Map<Integer, String> reasons = new HashMap<>();

    void init() {
        // Rows of a table written as calls: three or more statements with the same call.
        cars.add(new Car(1, "Entry_1", 1000d));
        cars.add(new Car(2, "Entry_2", 1500d));
        // a comment between rows
        cars.add(new Car(3, "Entry_3", 2000d));
        reasons.put(404, "Not Found");
        reasons.put(410, "Gone");
        reasons.put(418, "Teapot");
    }

    void tooShort(Map<Integer, String> m) {
        m.put(404, "Not Found");                              // expect: magic-number
        m.put(410, "Gone");                                   // expect: magic-number
    }

    void differentCalls(Map<Integer, String> m, List<Integer> l) {
        m.put(404, "Not Found");                              // expect: magic-number
        l.add(410);                                           // expect: magic-number
        m.remove(418);                                        // expect: magic-number
    }

    void differentReceivers(Map<Integer, String> a, Map<Integer, String> b, Map<Integer, String> c) {
        a.put(404, "Not Found");                              // expect: magic-number
        b.put(410, "Gone");                                   // expect: magic-number
        c.put(418, "Teapot");                                 // expect: magic-number
    }

    // Rows mixing in variables are operations on state, not data.
    void operations(Writer out, long inode, long mode, long size) {
        out.writeLong(inode, 8, 16);                          // expect: magic-number, magic-number
        out.writeLong(mode, 8, 16);                           // expect: magic-number, magic-number
        out.writeLong(size, 8, 16);                           // expect: magic-number, magic-number
    }

    void constantsAndBuilders(java.util.Calendar cal, List<Object> events) {
        cal.set(java.util.Calendar.HOUR_OF_DAY, 23);
        cal.set(java.util.Calendar.MINUTE, 59);
        cal.set(java.util.Calendar.SECOND, 59);
        events.add(Event.builder().name("a").start(java.time.LocalDate.of(2016, 2, 24)).build());
        events.add(Event.builder().name("b").start(java.time.LocalDate.of(2016, 2, 29)).build());
        events.add(Event.builder().name("c").start(java.time.LocalDate.of(2016, 3, 1)).build());
    }

    void interrupted(Map<Integer, String> m) {
        m.put(404, "Not Found");                              // expect: magic-number
        m.put(410, "Gone");                                   // expect: magic-number
        int limit = 500;                                      // expect: magic-number
        m.put(418, "Teapot");                                 // expect: magic-number
    }

    record Car(int id, String name, double price) {}
}
