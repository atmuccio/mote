import java.io.*;
import java.nio.file.Path;
import java.sql.Connection;
import java.sql.DriverManager;
import java.sql.PreparedStatement;
import java.sql.ResultSet;
import java.sql.SQLException;
import java.sql.Statement;
import java.util.Properties;
import java.util.Scanner;
import java.util.zip.ZipFile;

class Resources {
    String positives(File file, String url, Path path) throws IOException, SQLException {
        BufferedReader in = new BufferedReader(new FileReader(file));      // expect: resource-not-closed-with-try
        String first = in.readLine();
        in.close();                                                        // not on the exception path

        FileOutputStream out = new FileOutputStream(file);                 // expect: resource-not-closed-with-try
        out.write(1);

        PrintWriter pw = new PrintWriter("report.txt");                    // expect: resource-not-closed-with-try
        pw.println(first);

        Scanner sc = new Scanner(file);                                    // expect: resource-not-closed-with-try
        sc.nextLine();

        Scanner sp = new Scanner(path);                                    // expect: resource-not-closed-with-try
        sp.nextLine();

        ZipFile zip = new ZipFile(file);                                   // expect: resource-not-closed-with-try
        zip.size();

        RandomAccessFile raf = new RandomAccessFile(file, "r");            // expect: resource-not-closed-with-try
        raf.length();

        BufferedWriter bw = new BufferedWriter(new OutputStreamWriter(new FileOutputStream(file))); // expect: resource-not-closed-with-try
        bw.write("x");

        Connection conn = DriverManager.getConnection(url);                // expect: resource-not-closed-with-try
        PreparedStatement ps = conn.prepareStatement("select 1");          // expect: resource-not-closed-with-try
        ResultSet rs = ps.executeQuery();                                  // closed with `ps`
        Statement st = conn.createStatement();                             // expect: resource-not-closed-with-try
        ResultSet inline = conn.createStatement().executeQuery("select 2"); // expect: resource-not-closed-with-try
        inline.next();
        if (rs != null && rs.next()) {
            return rs.getString(1);
        }
        return first;
    }

    // ---- negatives
    String tryWithResources(File file) throws IOException {
        try (BufferedReader in = new BufferedReader(new FileReader(file))) {
            return in.readLine();
        }
    }

    void java9Resource(File file) throws IOException {
        FileInputStream in = new FileInputStream(file);
        try (in) {
            in.read();
        }
    }

    void closedInFinally(File file) throws IOException {
        FileInputStream in = new FileInputStream(file);
        try {
            in.read();
        } finally {
            in.close();
        }
    }

    void closedByHelperInFinally(File file) throws IOException {
        FileWriter w = new FileWriter(file);
        try {
            w.write("x");
        } finally {
            closeQuietly(w);
        }
    }

    InputStream returned(File file) throws IOException {
        FileInputStream in = new FileInputStream(file);
        return in;
    }

    private Reader field;

    void storedInField(File file) throws IOException {
        FileReader r = new FileReader(file);
        this.field = r;
    }

    Reader wrapped(File file) throws IOException {
        FileReader r = new FileReader(file);
        return new BufferedReader(r);
    }

    void passedOn(File file) throws IOException {
        FileInputStream in = new FileInputStream(file);
        Properties p = new Properties();
        p.load(in);                                                        // ownership unclear: silent
    }

    void captured(File file) throws IOException {
        FileInputStream in = new FileInputStream(file);
        Runnable r = () -> consume(in);
    }

    void notFiles(String text, Writer target) throws IOException {
        BufferedReader console = new BufferedReader(new InputStreamReader(System.in));
        console.readLine();
        BufferedReader strings = new BufferedReader(new StringReader(text));
        strings.readLine();
        PrintWriter wrapper = new PrintWriter(target);
        wrapper.println();
        PrintWriter sysout = new PrintWriter(System.out);
        sysout.flush();
        Scanner stdin = new Scanner(System.in);
        stdin.nextLine();
        StringWriter sw = new StringWriter();
        sw.write("x");
    }

    void unknownTypes(DataSource ds, String sql) throws SQLException {
        var conn = ds.getConnection();                                     // `var`: declared type unknown
        conn.isValid(1);
        MyConnection mine = ds.getConnection();                            // unknown type
        mine.ping();
    }

    ResultSet derivedReturned(Connection connection) throws SQLException {
        Statement stmt = connection.createStatement();
        return stmt.executeQuery("select 1");                              // the caller takes over
    }

    String resultSetOfManagedStatement(Connection connection) throws SQLException {
        try (Statement stmt = connection.createStatement()) {
            ResultSet rs = stmt.executeQuery("select 1");                  // closed with stmt
            return rs.next() ? rs.getString(1) : null;
        }
    }

    void aliasing(File file) throws IOException {
        FileInputStream in = new FileInputStream(file);
        InputStream alias = in;
        alias.close();
    }

    void nullInit() throws IOException {
        FileInputStream in = null;                                         // closed in finally: prefer-try-with-resources
        try {
            in = new FileInputStream("x");
        } finally {
            if (in != null) in.close();
        }
    }
}
