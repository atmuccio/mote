import com.google.common.base.Optional;
import com.acme.Option;

class GuavaFinder {
    Optional<String> find(String id) {
        return null;                                        // expect: optional-return-null
    }

    Option<String> other(String id) {
        return null;                                        // unknown library type
    }
}
