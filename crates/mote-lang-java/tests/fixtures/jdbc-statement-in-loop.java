import java.sql.*;
import java.util.List;

class Orders {
    static final String SHIPPED = "SHIPPED";

    void ship(Connection conn, List<Long> ids) throws SQLException {
        PreparedStatement ps = conn.prepareStatement("update orders set status = ? where id = ?");
        for (Long id : ids) {
            ps.setString(1, "SHIPPED");                            // expect: jdbc-statement-in-loop
            ps.setLong(2, id);
            ps.executeUpdate();                                    // expect: jdbc-statement-in-loop
        }
    }

    void constants(Connection conn, List<Long> ids) throws SQLException {
        try (PreparedStatement ps = conn.prepareStatement("insert into t values (?, ?, ?)")) {
            for (Long id : ids) {
                ps.setString(1, SHIPPED);                          // expect: jdbc-statement-in-loop
                ps.setNull(2, Types.VARCHAR);                      // expect: jdbc-statement-in-loop
                ps.setLong(3, id);
                ps.addBatch();
            }
            ps.executeBatch();
        }
    }

    void whileLoop(Connection conn, ResultSet rs) throws SQLException {
        PreparedStatement upd = conn.prepareStatement("update t set x = ? where id = ?");
        while (rs.next()) {
            upd.setInt(1, rs.getInt(1));
            upd.setInt(2, rs.getInt(2));
            upd.execute();                                         // expect: jdbc-statement-in-loop
        }
    }

    // Fine below.

    void batched(Connection conn, List<Long> ids) throws SQLException {
        PreparedStatement ps = conn.prepareStatement("update orders set status = ? where id = ?");
        for (Long id : ids) {
            ps.setLong(2, id);
            ps.executeUpdate();
        }
        ps.addBatch();
    }

    void preparedInLoop(Connection conn, List<Long> ids) throws SQLException {
        for (Long id : ids) {
            PreparedStatement ps = conn.prepareStatement("update orders set x = 1 where id = ?");
            ps.setLong(1, id);
            ps.executeUpdate();
        }
    }

    void reassigned(Connection conn, List<String> sqls) throws SQLException {
        PreparedStatement ps = null;
        for (String sql : sqls) {
            ps = conn.prepareStatement(sql);
            ps.setString(1, "X");
            ps.executeUpdate();
        }
    }

    void upsert(Connection conn, List<Long> ids) throws SQLException {
        PreparedStatement ps = conn.prepareStatement("update t set x = 1 where id = ?");
        for (Long id : ids) {
            ps.setLong(1, id);
            int n = ps.executeUpdate();
            if (n == 0) {
                insert(id);
            }
        }
    }

    void retry(Connection conn, long id) throws SQLException {
        PreparedStatement ps = conn.prepareStatement("update t set x = 1 where id = ?");
        for (int attempt = 0; attempt < 3; attempt++) {
            ps.setLong(1, id);
            ps.executeUpdate();
            break;
        }
    }

    void keys(Connection conn, List<Long> ids) throws SQLException {
        PreparedStatement ps = conn.prepareStatement("insert into t values (?)", Statement.RETURN_GENERATED_KEYS);
        for (Long id : ids) {
            ps.setLong(1, id);
            ps.executeUpdate();
            ResultSet keys = ps.getGeneratedKeys();
        }
    }

    void cleared(Connection conn, List<Long> ids) throws SQLException {
        PreparedStatement ps = conn.prepareStatement("update t set s = ? where id = ?");
        for (Long id : ids) {
            ps.clearParameters();
            ps.setString(1, "X");
            ps.setLong(2, id);
            ps.addBatch();
        }
    }

    void conditional(Connection conn, List<Long> ids) throws SQLException {
        PreparedStatement ps = conn.prepareStatement("update t set s = ? where id = ?");
        for (Long id : ids) {
            if (id > 10) {
                ps.setString(1, "BIG");
            } else {
                ps.setString(1, "SMALL");
            }
            ps.setString(1, "X");
            ps.setLong(2, id);
            ps.addBatch();
        }
    }

    void variable(Connection conn, List<Long> ids, String status) throws SQLException {
        PreparedStatement ps = conn.prepareStatement("update t set s = ? where id = ?");
        for (Long id : ids) {
            ps.setString(1, status);
            ps.setLong(2, id);
            ps.addBatch();
        }
    }

    void notJdbc(Statement2 st, List<Long> ids) {
        for (Long id : ids) {
            st.setString(1, "X");
            st.executeUpdate();
        }
    }

    void insert(long id) { }
}
