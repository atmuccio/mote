// The rule reads the text of a declaration before its tree: comments, annotations, longer names
// and strings holding the names it looks for must not change what it reports.
import java.io.*;

class TextFilters {
    void commentsAndAnnotations(File file) throws IOException {
        InputStream a = /* opened below */ null;                           // expect: resource-not-closed-with-try
        a = new FileInputStream(file);
        a.read();

        InputStream b /* = new FileInputStream(file) */;
        b = null;

        @Deprecated FileReader c = new FileReader(file);                   // expect: resource-not-closed-with-try
        c.read();

        // A word that only starts like a tracked class is not one.
        MyFileReader d = new MyFileReader(file);
        d.read();

        String e = "new FileInputStream(file)";
        e.length();
    }

    void longerNamesAndStrings(File file) throws IOException {
        FileInputStream in = new FileInputStream(file);                    // expect: resource-not-closed-with-try
        String input = "in";
        int inside = input.length();
        // in.close() in a comment is no close.
        in.read();
        in.close();
    }

    void escapeFoundBehindLongerNames(File file) throws IOException {
        FileInputStream in = new FileInputStream(file);
        String inbox = "in";
        keep(in);
        inbox.length();
    }

    void keep(InputStream s) {
    }

    static class MyFileReader {
        MyFileReader(File f) {
        }

        int read() {
            return 0;
        }
    }
}
