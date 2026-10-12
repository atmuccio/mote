import java.io.*;
import java.util.zip.ZipOutputStream;

class Report {
    void write(File f, String text) throws IOException {
        try (BufferedWriter out = new BufferedWriter(new FileWriter(f))) {
            out.write(text);
            out.close();                                           // expect: redundant-close-in-try-with-resources
        }
    }

    void two(File a, File b) throws IOException {
        try (InputStream in = new FileInputStream(a); OutputStream out = new FileOutputStream(b)) {
            in.transferTo(out);
            in.close();                                            // expect: redundant-close-in-try-with-resources
            // done
        } catch (IOException e) {
            throw e;
        }
    }

    void java9(InputStream existing) throws IOException {
        try (existing) {
            existing.read();
            existing.close();                                      // expect: redundant-close-in-try-with-resources
        }
    }

    byte[] earlyClose() throws IOException {
        ByteArrayOutputStream bytes = new ByteArrayOutputStream();
        try (ZipOutputStream zip = new ZipOutputStream(bytes)) {
            zip.finish();
            zip.close();                                           // closing early has an effect: fine
            return bytes.toByteArray();
        }
    }

    void otherObject(File f, Closeable other) throws IOException {
        try (InputStream in = new FileInputStream(f)) {
            in.read();
            other.close();                                         // not a resource: fine
        }
    }

    void plainTry(File f) throws IOException {
        InputStream in = new FileInputStream(f);
        try {
            in.read();
            in.close();
        } finally {
            in.close();
        }
    }

    void nested(File f) throws IOException {
        try (InputStream in = new FileInputStream(f)) {
            if (in.available() > 0) {
                in.close();
            }
        }
    }
}

class CloseTests {
    void closeTwice(File f) throws IOException {
        try (InputStream in = new FileInputStream(f)) {
            in.read();
            in.close();
            in.close();
        }
    }

    void onlyClose(InputStream decoder) throws IOException {
        try (InputStream in = new BufferedInputStream(decoder)) {
            in.close();
        }
    }
}
