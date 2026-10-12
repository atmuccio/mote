// mote-test: test-file
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.io.File;
import org.junit.jupiter.api.Test;

class ArchiveIT {
    interface Check {
        void run(File f) throws Exception;
    }

    private static final Check HAS_ENTRIES = f -> assertTrue(f.length() > 0);

    private static final Check SILENT = f -> f.delete();

    private final Runnable notALambda = null;

    @Test
    void lambdaConstant() throws Exception {
        withArchive("a", HAS_ENTRIES);
    }

    @Test
    void lambdaConstantWithoutAssertion() throws Exception {   // expect: test-without-assertion
        withArchive("b", SILENT);
    }

    @Test
    void testerChain() {
        Type4Tester.create("1 2 add").pop(3).isEmpty();
    }

    @Test
    void testerVariable() {
        ResultVerifier verifier = new ResultVerifier();
        verifier.check("x");
    }

    @Test
    void plainObject() {                                        // expect: test-without-assertion
        Calculator.create("1 2 add").compute();
    }

    private void withArchive(String name, Check check) throws Exception {
        File f = new File(name);
        check.run(f);
    }
}
