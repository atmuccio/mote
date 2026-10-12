// mote-test: path src/main/java/com/acme/Reports.java
import java.io.File;
import java.io.FileInputStream;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.Optional;

class Reports {
    private static final String OUTPUT = "C:\\reports\\daily";                   // expect: hard-coded-absolute-path
    private static final String EXPORT_DIR = "D:/exports";                      // expect: hard-coded-absolute-path
    private static final String HOME_DIR = "/Users/alice/.reports";             // expect: hard-coded-absolute-path

    void write(String name) throws Exception {
        File out = new File("C:\\reports\\" + name + ".pdf");                   // expect: hard-coded-absolute-path
        Path p = Paths.get("/Users/bob/work", name);                            // expect: hard-coded-absolute-path
        Path q = Path.of("/var/tmp/reports");                                   // expect: hard-coded-absolute-path
        FileInputStream in = new FileInputStream("/tmp/keys/signing.key");      // expect: hard-coded-absolute-path
        File font = new File("C:\\Windows\\Fonts\\arial.ttf");                  // expect: hard-coded-absolute-path
        use(("e:/scratch/" + name));                                            // expect: hard-coded-absolute-path
        save("c:/users/carol/downloads/out.pdf");                               // expect: hard-coded-absolute-path
    }

    // Compliant
    private static final String FONTS_WIN = "C:\\Windows\\Fonts";               // a platform table
    private static final String FONTS_MAC = "/Library/Fonts";
    private static final String REMOTE_HOME = "/tmp/remote-shell-%s/";          // a path on another host
    private String cacheDir = "/tmp/app-cache";                                 // a default kept in a field

    @Deprecated(since = "C:\\old")
    void fine(String path, String route, Optional<String> configured) throws Exception {
        boolean windows = path.startsWith("C:\\");                              // a bare root
        boolean onDrive = path.startsWith("C:\\Windows");                       // path-parsing code
        boolean same = "D:/exports".equals(path);                               // compared, not used
        boolean eq = path == "/tmp/x";
        String dir = System.getProperty("app.dir", "C:\\data");                 // a configurable default
        String other = configured.orElse("D:/fallback");
        String folder;
        folder = "/tmp";
        String home = "/Users/";                                                // a bare root
        String page = "/Users/list";                                            // not a path-named variable
        route = "/Users/list";
        File container = new File("/home/app/data");                           // container layout
        File service = new File("/root/.config");
        String uri = "file:///C:/data/x.txt";                                   // a URI
        String msg = "/tmp is full";                                            // a message
        String rel = "reports/daily";
        String unix = "/etc/app.conf";                                          // not a temporary folder
        String dollars = "$:/x";
        switch (path) {
            case "C:\\data":
                break;
            default:
                break;
        }
        String block = """
            C:\\data
            """;
    }

    @Value("/tmp/uploads")
    String uploads;

    static void use(String s) {
    }

    static void save(String s) {
    }

    @interface Value {
        String value();
    }
}
