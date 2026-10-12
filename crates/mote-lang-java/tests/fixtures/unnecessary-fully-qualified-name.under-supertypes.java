// mote-test: params static_imports_under_supertypes=true
import static com.acme.util.Strings.isBlank;

import com.acme.util.Strings;

class Report extends com.acme.base.Page {
    boolean empty(String s) {
        return Strings.isBlank(s);                                  // expect: unnecessary-fully-qualified-name
    }
}

class Listeners {
    Runnable r = new Runnable() {
        public void run() {
            // Anonymous classes stay exempt: their supertype is always another type.
            Strings.isBlank("x");
        }
    };
}
