// mote-test: test-file
import java.io.File;
import java.io.FileInputStream;
import java.sql.Connection;
import java.sql.Statement;

class ResourceTest {
    void stubs(Connection connection) throws Exception {
        Statement statement = connection.createStatement();                // JDBC objects in tests are mocks
        when(statement.executeQuery("select 1")).thenReturn(null);
    }

    void files(File f) throws Exception {
        FileInputStream in = new FileInputStream(f);                       // expect: resource-not-closed-with-try
        in.read();
    }
}
