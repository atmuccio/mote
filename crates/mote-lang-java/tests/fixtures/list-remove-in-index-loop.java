import java.util.ArrayList;
import java.util.List;

class Cleanup {
    private final List<String> names = new ArrayList<>();

    void wrong(List<Integer> ids) {
        for (int i = 0; i < names.size(); i++) {
            if (names.get(i).isBlank()) {
                names.remove(i);                                    // expect: list-remove-in-index-loop
            }
        }
        for (int k = 0; k < ids.size(); ++k) {
            if (ids.get(k) < 0) {
                ids.remove(k);                                      // expect: list-remove-in-index-loop
                log(k);
            }
        }
    }

    void fine(List<Integer> ids, Registry registry) {
        for (int i = 0; i < names.size(); i++) {
            if (names.get(i).isBlank()) {
                names.remove(i);
                i--;
            }
        }
        for (int i = names.size() - 1; i >= 0; i--) {
            if (names.get(i).isBlank()) {
                names.remove(i);
            }
        }
        for (int i = 0; i < names.size(); i++) {
            if (names.get(i).isBlank()) {
                names.remove(i);
                break;
            }
        }
        for (int i = 0; i < names.size(); i++) {
            if (names.get(i).isEmpty()) {
                names.remove(i);
                names.add(i, "-");
            }
        }
        for (int i = 0; i < names.size(); i++) {
            if (names.get(i).isEmpty()) {
                return;
            }
        }
        for (Integer i = 0; i < ids.size(); i++) {
            ids.remove(i);
        }
        for (int i = 0; i < 3; i++) {
            registry.remove(i);
        }
        for (int i = 0; i < names.size(); i++) {
            String removed = names.get(i);
            if (removed.isEmpty()) {
                return;
            }
        }
        names.removeIf(String::isBlank);
    }

    void log(int k) { }

    interface Registry { void remove(int i); }
}
