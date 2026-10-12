import java.io.BufferedReader;
import java.io.File;
import java.io.FileInputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.sql.CallableStatement;
import java.sql.Connection;
import java.sql.DatabaseMetaData;
import java.sql.PreparedStatement;
import java.sql.ResultSet;
import java.sql.SQLException;
import java.util.List;
import java.util.stream.Collectors;
import java.util.stream.Stream;

class NioFiles {
    long count(Path path) throws IOException {
        Stream<String> lines = Files.lines(path);                          // expect: resource-not-closed-with-try
        long n = lines.count();
        return n;
    }

    void copy(Path from, Path to) throws IOException {
        InputStream in = Files.newInputStream(from);                       // expect: resource-not-closed-with-try
        OutputStream out = Files.newOutputStream(to);                      // expect: resource-not-closed-with-try
        in.transferTo(out);
        in.close();
        out.close();
    }

    String firstLine(Path path) throws IOException {
        BufferedReader r = Files.newBufferedReader(path);                  // expect: resource-not-closed-with-try
        String line = r.readLine();
        return line;
    }

    List<String> managed(Path path) throws IOException {
        try (Stream<String> lines = Files.lines(path)) {
            return lines.collect(Collectors.toList());
        }
    }

    Stream<Path> handedOver(Path dir) throws IOException {
        Stream<Path> entries = Files.list(dir);
        return entries;                                                    // the caller closes it
    }

    List<String> readAll(Path path) throws IOException {
        List<String> all = Files.readAllLines(path);                       // not a resource
        return all;
    }
}

class Metadata {
    void tables(Connection connection) throws SQLException {
        DatabaseMetaData meta = connection.getMetaData();
        ResultSet tables = meta.getTables(null, null, "%", null);          // expect: resource-not-closed-with-try
        while (tables.next()) {
            tables.getString(3);
        }
        ResultSet types = connection.getMetaData().getTableTypes();        // expect: resource-not-closed-with-try
        types.next();
        try (ResultSet cols = meta.getColumns(null, null, "T", "%")) {
            cols.next();
        }
    }

    void call(Connection connection) throws SQLException {
        CallableStatement cs = connection.prepareCall("{call p()}");        // expect: resource-not-closed-with-try
        cs.execute();
    }

    void unknownReceiver(Catalog catalog) throws SQLException {
        ResultSet rs = catalog.getTables(null, null, "%", null);           // receiver type unknown
        rs.next();
    }
}

class DeclaredNull {
    void finallyClose(File file) throws IOException {
        FileInputStream in = null;                                          // closed in finally: not a leak
        try {
            in = new FileInputStream(file);
            in.read();
        } finally {
            if (in != null) {
                in.close();
            }
        }
    }

    void helperInFinally(Connection connection, String sql) throws SQLException {
        PreparedStatement ps = null;                                        // closed by a helper in finally
        ResultSet rs = null;
        try {
            ps = connection.prepareStatement(sql);
            rs = ps.executeQuery();                                         // closed with ps
            rs.next();
        } finally {
            closeQuietly(rs);
            closeQuietly(ps);
        }
    }

    void neverClosed(File file) throws IOException {
        InputStream in = null;                                              // expect: resource-not-closed-with-try
        in = new FileInputStream(file);
        in.read();
        in.close();
    }

    InputStream returned(File file) throws IOException {
        InputStream in = null;
        try {
            in = new FileInputStream(file);
            return in;
        } catch (IOException e) {
            if (in != null) {
                in.close();
            }
            throw e;
        }
    }

    void mixedSources(File file, boolean useStdin) throws IOException {
        InputStream in = null;
        try {
            if (useStdin) {
                in = System.in;
            } else {
                in = new FileInputStream(file);
            }
            in.read();
        } finally {
            if (in != null && in != System.in) {
                in.close();
            }
        }
    }

    void neverAssigned() throws IOException {
        InputStream in = null;
        if (in != null) {
            in.close();
        }
    }

    void handedToField(File file) throws IOException {
        InputStream in = null;
        in = new FileInputStream(file);
        this.stream = in;
    }

    void passedOn(File file) throws IOException {
        InputStream in = null;
        try {
            in = new FileInputStream(file);
            consume(in);
        } finally {
            if (in != null) {
                in.close();
            }
        }
    }

    void resultSetOfVariable(PreparedStatement ps) throws SQLException {
        ResultSet rs = null;
        try {
            rs = ps.executeQuery();                                         // closed with ps
            rs.next();
        } finally {
            if (rs != null) {
                rs.close();
            }
        }
    }

    void lambdaCapture(File file) throws IOException {
        InputStream in = null;
        in = new FileInputStream(file);
        final InputStream copy = in;
        Runnable r = () -> consume(copy);
    }

    private InputStream stream;
    void consume(InputStream in) { }
    void closeQuietly(AutoCloseable c) { }
}
