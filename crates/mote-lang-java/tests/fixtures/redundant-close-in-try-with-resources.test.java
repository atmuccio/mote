// mote-test: test-file
import java.io.*;

class StreamTest {
    void testCloseIsDelegated(File f) throws IOException {
        try (InputStream in = new FileInputStream(f)) {
            in.read();
            in.close();
        }
    }

    void testRead(File f) throws IOException {
        try (InputStream in = new FileInputStream(f)) {
            in.read();
            in.close();                                       // expect: redundant-close-in-try-with-resources
        }
    }
}
