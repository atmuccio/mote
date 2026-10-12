// mote-test: test-file
import static org.junit.jupiter.api.Assertions.assertThrows;

import java.io.File;
import java.io.FileInputStream;
import java.io.StringReader;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import org.junit.jupiter.api.Test;

class UploaderTest {
    @Test
    void checkedSetup(Uploader uploader, Path dir, Class<?> type) {
        assertThrows(UploadException.class, () -> uploader.upload(File.createTempFile("a", ".pdf")));     // expect: assert-throws-multiple-calls
        assertThrows(UploadException.class, () -> uploader.upload(Files.readString(dir)));                 // expect: assert-throws-multiple-calls
        assertThrows(UploadException.class, () -> uploader.send(new FileInputStream("in.txt")));          // expect: assert-throws-multiple-calls
        assertThrows(UploadException.class, () -> uploader.load(Class.forName("com.acme.Doc")));           // expect: assert-throws-multiple-calls
        assertThrows(UploadException.class, () -> uploader.call(type.getMethod("run")));                   // expect: assert-throws-multiple-calls
    }

    @Test
    void harmlessJdk(Uploader uploader, Path dir, String text) {
        assertThrows(UploadException.class, () -> uploader.check(Files.exists(dir)));
        assertThrows(UploadException.class, () -> uploader.list(List.of(text)));
        assertThrows(UploadException.class, () -> uploader.read(new StringReader(text)));
        assertThrows(UploadException.class, () -> uploader.upload(dir.resolve(text)));
    }
}
