// A command-line program: console output is its user interface.
class AdminTool {
    void printUsage() {
        System.out.println("usage: admin <command>");
        System.err.println("  --help   show this help");
    }

    public static void main(String[] args) {
        new AdminTool().printUsage();
        System.exit(args.length == 0 ? 1 : 0);
    }
}
