class Connection {
    private static String url;
    private static Connection current;
    private static int created;
    private static long bytes;
    private static final Object LOCK = new Object();
    private static Connection registered;
    private String name;

    Connection(String address, String name) {
        url = address;                                     // expect: static-field-set-in-constructor
        Connection.current = this;                         // expect: static-field-set-in-constructor
        this.name = name;
        created++;
        bytes += 10;
        synchronized (LOCK) {
            registered = this;
        }
        Runnable r = () -> {
            url = "lambda";
        };
    }

    Connection(int port) {
        String url = "local:" + port;
        url = url.trim();
        this.name = url;
    }

    static void configure(String u) {
        url = u;
    }
}

class Shadowing {
    private static int level;

    Shadowing(int level) {
        level = level + 1;
    }

    class Inner {
        Inner() {
            level = 3;                                         // enclosing class's static field
        }
    }
}

class Documented {
    private static boolean debug;
    private static int timeout;
    private static String host;

    Documented(boolean d) {
        debug = d; // the whole service shares one debug switch
        host = "localhost"; // remote-host override comes later; one for all connections
        // one timeout for all handlers, re-read on each construction
        timeout = 3;
    }
}

class Singleton {
    private static String home;
    private static final Singleton INSTANCE = new Singleton();

    private Singleton() {
        home = System.getProperty("user.home");
    }
}
