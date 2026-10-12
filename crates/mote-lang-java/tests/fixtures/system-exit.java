import static java.lang.System.exit;

class Exits {
    void fail() {
        System.exit(1);                                          // expect: system-exit
        java.lang.System.exit(2);                                // expect: system-exit
        Runtime.getRuntime().exit(3);                            // expect: system-exit
        Runtime.getRuntime().halt(4);                            // expect: system-exit
        Runtime rt = Runtime.getRuntime();
        rt.halt(5);                                              // expect: system-exit
        exit(6);                                                 // expect: system-exit
    }

    public static void main(String[] args) {
        if (args.length == 0) {
            System.exit(1);
        }
        Runtime.getRuntime().addShutdownHook(new Thread() {
            public void run() {
                Runtime.getRuntime().halt(0);
            }
        });
        Runnable r = () -> System.exit(0);
    }

    void main() {
        System.exit(0);                                          // expect: system-exit
    }

    void unrelated(Door door, Shell shell) {
        door.exit();
        shell.halt();
        exitApp();
    }

    void exitApp() {}
    interface Door { void exit(); }
    interface Shell { void halt(); }
}

class OwnExit {
    void exit(int code) {}
    void m() {
        exit(1);                                                 // this class's own method
    }
}
