// mote-test: params report_parameters=true
import java.util.Optional;

class Service {
    void update(Optional<String> name) {}                      // expect: optional-null

    Service(Optional<Integer> limit) {}                         // expect: optional-null

    void plain(String name) {}

    void bound(@Deprecated Optional<String> q) {}

    @Override
    public boolean equals(Object o) { return false; }

    interface Api { void call(Optional<String> x); }          // expect: optional-null
    static class Impl implements Api {
        @Override
        public void call(Optional<String> x) {}
    }
}
