import java.io.IOException;
import java.io.FileNotFoundException;
import java.sql.SQLException;

class Loader {
    void load(String file) {
        try {
            read(file);
        } catch (IOException e) {
            log("cannot load " + file, e);
        } catch (SQLException e) {                                  // expect: identical-catch-branches
            log("cannot load " + file, e);
        }

        try {
            read(file);
        } catch (IOException ioe) {
            log("cannot load " + file, ioe);
        } catch (SQLException sqle) {                               // expect: identical-catch-branches
            log("cannot load " + file, sqle);
        }

        try {
            read(file);
        } catch (FileNotFoundException e) {
            log("failed", e);
        } catch (IOException e) {                                   // subclass: cannot merge
            log("failed", e);
        }

        try {
            read(file);
        } catch (IOException e) {
            log("io", e);
        } catch (SQLException e) {
            log("sql", e);
        }

        try {
            read(file);
        } catch (IOException e) {
        } catch (SQLException e) {                                  // empty: another rule
        }

        try {
            read(file);
        } catch (ProjectException e) {
            log("failed", e);
        } catch (OtherProjectException e) {                         // unknown types: cannot prove unrelated
            log("failed", e);
        }

        try {
            read(file);
        } catch (IOException e) {
            // io
            retry();
        } catch (SQLException e) {                                  // expect: identical-catch-branches
            /* sql */
            retry();
        }
    }

    void read(String f) throws IOException, SQLException { }

    void log(String m, Exception e) { }

    void retry() { }
}
