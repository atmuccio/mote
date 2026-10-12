import java.io.PrintStream;

class Printing {
    void m(Exception e, PrintStream ps) {
        System.out.println("hello");                             // expect: system-out-print
        System.err.print("oops");                                // expect: system-out-print
        System.out.printf("%d%n", 1);                            // expect: system-out-print
        java.lang.System.out.println();                          // expect: system-out-print
        System.err.format("x");                                  // expect: system-out-print
        System                                                   // expect: system-out-print
            .out
            .println("multi-line");
        System.out.flush();
        e.printStackTrace(System.err);
        ps.println("to a stream");
        PrintStream out = System.out;
        out.println("aliased");                                  // aliases are not tracked
        System.setOut(ps);
    }

    void main() {                                                // not a static main
        System.out.println("usage: tool <file>");                // expect: system-out-print
    }
}

class Shadowing {
    static class System {
        static final Out out = new Out();
    }
    static class Out {
        void println(String s) {}
    }
    void m() {
        System.out.println("not java.lang.System");
    }
}
