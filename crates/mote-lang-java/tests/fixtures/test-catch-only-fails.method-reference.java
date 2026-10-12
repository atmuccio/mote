// mote-test: test-file
import static org.junit.jupiter.api.Assertions.fail;

import java.io.IOException;
import java.util.ArrayList;
import java.util.List;
import org.junit.jupiter.api.Test;

// Test bodies registered as method references of a functional interface declared elsewhere.
abstract class OptionsProviderBase {
    private final List<OptionCase> cases = new ArrayList<>();

    protected OptionsProviderBase() {
        cases.add(OptionCase.named("help", this::helpTest));
        cases.add(OptionCase.named("style", this::styleTest));
        cases.add(OptionCase.named("dir", () -> System.out.println("no test")));
    }

    protected void helpTest() {
        try {
            generate("--help");
        } catch (IOException e) {
            fail(e.getMessage());
        }
        prepareOutput();
    }

    private void prepareOutput() {
        try {
            copyResource("out");
        } catch (IOException e) {
            fail(e.getMessage());
        }
    }

    protected void styleTest() {
        styleTest("stylesheet");
    }

    // One level below a referenced method: the exception has nowhere to go either.
    private void styleTest(String option) {
        try {
            copyResource(option);
        } catch (IOException e) {
            fail("Could not copy the resource: " + e.getMessage());
        }
    }

    // Private, but also called from a plain test method.
    private void sharedHelper() {
        try {
            generate("--shared");
        } catch (IOException e) {
            fail(e.getMessage());                                    // expect: test-catch-only-fails
        }
    }

    protected void notReferenced() {
        sharedHelper();
        try {
            generate("--version");
        } catch (IOException e) {
            fail(e.getMessage());                                    // expect: test-catch-only-fails
        }
    }

    @Test
    void plainTest() {
        sharedHelper();
    }

    abstract void generate(String arg) throws IOException;

    abstract void copyResource(String name) throws IOException;

    interface OptionCase {
        static OptionCase named(String name, Runnable body) {
            return null;
        }
    }
}
