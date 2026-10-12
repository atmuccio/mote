// mote-test: test-file
import java.io.File;
import java.io.FileInputStream;
import java.io.IOException;
import java.sql.Connection;
import java.sql.PreparedStatement;
import java.sql.SQLException;

class CleanupTest {
    void files(File file) throws IOException {
        FileInputStream in = null;                                          // expect: prefer-try-with-resources
        try {
            in = new FileInputStream(file);
            in.read();
        } finally {
            if (in != null) {
                in.close();
            }
        }
    }

    void jdbc(Connection connection) throws SQLException {
        PreparedStatement ps = null;                                        // JDBC in tests: usually mocks
        try {
            ps = connection.prepareStatement("select 1");
            ps.execute();
        } finally {
            if (ps != null) {
                ps.close();
            }
        }
    }
}
