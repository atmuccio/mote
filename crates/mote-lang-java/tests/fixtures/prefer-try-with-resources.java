import java.io.BufferedReader;
import java.io.File;
import java.io.FileInputStream;
import java.io.FileReader;
import java.io.FileWriter;
import java.io.IOException;
import java.io.InputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.sql.Connection;
import java.sql.DatabaseMetaData;
import java.sql.PreparedStatement;
import java.sql.ResultSet;
import java.sql.SQLException;
import java.sql.Statement;

class Cleanup {
    void declaredNull(File file) throws IOException {
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

    void initialized(File file) throws IOException {
        BufferedReader reader = new BufferedReader(new FileReader(file));   // expect: prefer-try-with-resources
        try {
            reader.readLine();
        } finally {
            reader.close();
        }
    }

    void helperInFinally(Connection connection, String sql) throws SQLException {
        PreparedStatement ps = null;                                        // expect: prefer-try-with-resources
        ResultSet rs = null;                                                // result of ps: closed with it
        try {
            ps = connection.prepareStatement(sql);
            rs = ps.executeQuery();
            rs.next();
        } finally {
            closeQuietly(rs);
            closeQuietly(ps);
        }
    }

    void metadata(Connection connection) throws SQLException {
        DatabaseMetaData meta = connection.getMetaData();
        ResultSet tables = null;                                            // expect: prefer-try-with-resources
        try {
            tables = meta.getTables(null, null, "%", null);
            tables.next();
        } finally {
            if (tables != null) {
                try {
                    tables.close();
                } catch (SQLException ignored) {
                    // best effort
                }
            }
        }
    }

    void nio(Path path) throws IOException {
        InputStream in = Files.newInputStream(path);                        // expect: prefer-try-with-resources
        try {
            in.read();
        } finally {
            in.close();
        }
    }

    // ---- negatives

    void alreadyManaged(File file) throws IOException {
        try (FileInputStream in = new FileInputStream(file)) {
            in.read();
        }
    }

    void java9Resource(File file) throws IOException {
        FileInputStream in = new FileInputStream(file);
        try (in) {
            in.read();
        }
    }

    void neverClosedInFinally(File file) throws IOException {
        FileWriter w = new FileWriter(file);                                // a leak: the other rule
        w.write("x");
        w.close();
    }

    InputStream returned(File file) throws IOException {
        InputStream in = null;
        try {
            in = new FileInputStream(file);
            return in;
        } finally {
            if (in == null) {
                closeQuietly(in);
            }
        }
    }

    void passedOnInTry(File file) throws IOException {
        InputStream in = null;
        try {
            in = new FileInputStream(file);
            consume(in);                                                    // ownership may move
        } finally {
            if (in != null) {
                in.close();
            }
        }
    }

    void mixedSources(File file, boolean stdin) throws IOException {
        InputStream in = null;
        try {
            in = stdin ? System.in : new FileInputStream(file);
            in.read();
        } finally {
            if (in != null && in != System.in) {
                in.close();
            }
        }
    }

    void storedInField(File file) throws IOException {
        InputStream in = null;
        try {
            in = new FileInputStream(file);
            this.last = in;
        } finally {
            if (in != null) {
                in.close();
            }
        }
    }

    void capturedByLambda(File file) throws IOException {
        InputStream in = new FileInputStream(file);
        try {
            Runnable r = () -> consume(in);
            r.run();
        } finally {
            in.close();
        }
    }

    void resultSetOfVariable(Statement st) throws SQLException {
        ResultSet rs = null;
        try {
            rs = st.executeQuery("select 1");                               // closed with st
            rs.next();
        } finally {
            if (rs != null) {
                rs.close();
            }
        }
    }

    void notAResource(StringBuilder out) {
        StringBuilder sb = null;
        try {
            sb = new StringBuilder();
            out.append(sb);
        } finally {
            sb = null;
        }
    }

    void unknownType(Pool pool) throws Exception {
        Handle h = null;
        try {
            h = pool.acquire();
        } finally {
            if (h != null) {
                h.close();
            }
        }
    }

    private InputStream last;
    void consume(InputStream in) { }
    void closeQuietly(AutoCloseable c) { }
}
