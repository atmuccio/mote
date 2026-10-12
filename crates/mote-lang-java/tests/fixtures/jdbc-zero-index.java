import java.sql.CallableStatement;
import java.sql.Connection;
import java.sql.PreparedStatement;
import java.sql.ResultSet;
import java.sql.ResultSetMetaData;
import java.sql.SQLException;
import java.sql.Types;

class Dao {
    void wrong(Connection con, String name) throws SQLException {
        PreparedStatement ps = con.prepareStatement("select id from t where name = ?");
        ps.setString(0, name);                                  // expect: jdbc-zero-index
        ResultSet rs = ps.executeQuery();
        long id = rs.getLong(0);                                // expect: jdbc-zero-index
        ResultSetMetaData md = rs.getMetaData();
        String col = md.getColumnName(0);                       // expect: jdbc-zero-index
        CallableStatement cs = con.prepareCall("{call f(?)}");
        cs.registerOutParameter(0, Types.INTEGER);              // expect: jdbc-zero-index
        rs.updateString(0, "x");                                // expect: jdbc-zero-index
    }

    void fine(PreparedStatement ps, ResultSet rs, Json json, String name) throws SQLException {
        ps.setString(1, name);
        ps.setFetchSize(0);
        ps.setMaxRows(0);
        ps.setQueryTimeout(0);
        long id = rs.getLong(1);
        String s = rs.getString("name");
        rs.absolute(0);
        Object o = json.getString(0);
        for (int i = 0; i < 3; i++) {
            ps.setInt(i + 1, i);
        }
    }

    interface Json { Object getString(int i); }
}
