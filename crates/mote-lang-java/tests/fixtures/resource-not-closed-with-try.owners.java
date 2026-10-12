import com.zaxxer.hikari.HikariDataSource;
import java.sql.Connection;
import java.sql.DriverManager;
import java.sql.PreparedStatement;
import java.sql.ResultSet;
import java.sql.SQLException;
import java.sql.Statement;
import javax.sql.DataSource;
import org.springframework.jdbc.datasource.DataSourceUtils;

class Owners {
    // ---- connections the caller owns: still reported
    void fromDataSource(DataSource ds) throws SQLException {
        Connection conn = ds.getConnection();                               // expect: resource-not-closed-with-try
        conn.setAutoCommit(false);
        conn.close();
    }

    void fromLibraryDataSource(HikariDataSource pool) throws SQLException {
        Connection conn = pool.getConnection();                             // expect: resource-not-closed-with-try
        conn.isValid(1);
    }

    void fromLocalDataSource(RoutingDataSource routing) throws SQLException {
        Connection conn = routing.getConnection();                          // expect: resource-not-closed-with-try
        conn.isValid(1);
    }

    void fromDriverManager(String url) throws SQLException {
        Connection conn = DriverManager.getConnection(url);                 // expect: resource-not-closed-with-try
        conn.isValid(1);
        conn.close();
    }

    // ---- connections lent by an owner that manages them: not reported
    String catalogOf(UnitOfWork work) throws SQLException {
        Connection c = ((JdbcUnitOfWork) work).getConnection();
        return c.getCatalog();
    }

    void fromSession(Session session) throws SQLException {
        Connection c = session.getConnection();
        c.setReadOnly(true);
    }

    void fromStatement(Statement st) throws SQLException {
        Connection c = st.getConnection();
        c.getCatalog();
    }

    void fromOwnAccessor() throws SQLException {
        Connection c = getConnection();
        c.getCatalog();
    }

    void fromTransactionHelper(DataSource ds) throws SQLException {
        Connection c = DataSourceUtils.getConnection(ds);
        c.getCatalog();
    }

    void assignedLater(Session session) throws SQLException {
        Connection c = null;
        c = session.getConnection();
        c.getCatalog();
    }

    // ---- closed by the very next statement: not reported
    void probe(DataSource ds) throws SQLException {
        Connection conn = ds.getConnection();
        conn.close();
    }

    void probeWithComment(DataSource ds) throws SQLException {
        Connection conn = ds.getConnection();
        // only checks that the pool hands out connections
        conn.close();
    }

    void notRightAfter(DataSource ds) throws SQLException {
        Connection conn = ds.getConnection();                               // expect: resource-not-closed-with-try
        conn.setAutoCommit(true);
        conn.close();
    }

    void twoDeclarators(DataSource ds) throws SQLException {
        Connection first = ds.getConnection(), second = ds.getConnection(); // expect: resource-not-closed-with-try
        second.close();
        first.close();
    }

    // ---- statements whose connection is closed on the failure path: not reported
    Connection openChecked(String url, String sql) {
        Connection con = null;
        try {
            con = DriverManager.getConnection(url);
            Statement st = con.createStatement();
            st.execute(sql);
            st.close();
            return con;
        } catch (SQLException e) {
            closeQuietly(con);
            return null;
        }
    }

    Connection openCheckedNested(String url, String sql) {
        Connection con = null;
        try {
            con = DriverManager.getConnection(url);
            if (sql != null) {
                PreparedStatement ps = con.prepareStatement(sql);
                ps.execute();
                ps.close();
            }
            return con;
        } catch (SQLException e) {
            try {
                if (con != null) {
                    con.close();
                }
            } catch (SQLException ignored) {
                return null;
            }
            return null;
        }
    }

    void closedInFinally(DataSource ds) throws SQLException {
        Connection conn = ds.getConnection();
        try {
            PreparedStatement ps = conn.prepareStatement("update t set x = 1");
            ps.executeUpdate();
        } finally {
            conn.close();
        }
    }

    void managedConnection(DataSource ds) throws SQLException {
        try (Connection conn = ds.getConnection()) {
            Statement st = conn.createStatement();
            ResultSet rs = conn.createStatement().executeQuery("select 1");
            ResultSet tables = conn.getMetaData().getTables(null, null, "%", null);
            st.execute("select 1");
            rs.next();
            tables.next();
        }
    }

    void java9Resource(Connection conn) throws SQLException {
        try (conn) {
            Statement st = conn.createStatement();
            st.execute("select 1");
        }
    }

    // ---- the connection is not closed on the failure path of the statement: still reported
    void closedOnlyOnSuccess(DataSource ds) throws SQLException {
        Connection conn = ds.getConnection();                               // expect: resource-not-closed-with-try
        Statement st = conn.createStatement();                              // expect: resource-not-closed-with-try
        st.execute("select 1");
        conn.close();
    }

    void closedByAnotherTry(Connection conn) throws SQLException {
        Statement st = conn.createStatement();                              // expect: resource-not-closed-with-try
        st.execute("select 1");
        try {
            conn.commit();
        } catch (SQLException e) {
            conn.close();
        }
    }

    void closesAnotherVariable(Connection conn, Connection other) throws SQLException {
        try {
            Statement st = conn.createStatement();                          // expect: resource-not-closed-with-try
            st.execute("select 1");
        } catch (SQLException e) {
            other.close();
            throw e;
        }
    }

    Connection getConnection() {
        return null;
    }

    void closeQuietly(AutoCloseable c) {
    }

    interface RoutingDataSource {
        Connection getConnection() throws SQLException;
    }
}
