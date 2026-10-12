// mote-test: test-file
import org.junit.jupiter.api.Nested;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.params.ParameterizedTest;

class CartTest {
    @Test
    void startsEmpty() {}

    class WhenEmpty {                       // expect: nested-test-class-not-run
        @Test
        void totalIsZero() {}
    }

    public class WithItems {                // expect: nested-test-class-not-run
        @ParameterizedTest
        void totals(int n) {}
    }

    @Nested
    class WhenFull {
        @Test
        void rejectsMore() {}

        class Deeper {                      // expect: nested-test-class-not-run
            @Test
            void stillSkipped() {}
        }
    }

    static class StaticHolder {             // static: depends on the build tool
        @Test
        void runsOrNot() {}
    }

    abstract class Template {               // template for @Nested subclasses
        @Test
        void shared() {}
    }

    class Helper {                          // no tests
        void assist() {}
    }

    interface Contract {
        @Test
        default void holds() {}
    }

    class Junit4Style {
        @org.junit.Test
        public void notJupiter() {}
    }
}
