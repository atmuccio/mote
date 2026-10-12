class Printer {
    String render(String line, int width) {
        return header(line, width) + footer("x", 1);
    }

    private String header(String line, int width) {                // expect: unused-private-method-parameter
        // the width is ignored here: see the javadoc of line
        return System.getProperty("line.separator") + line.trim();
    }

    private String footer(String text, int line) {                  // expect: unused-private-method-parameter
        /* line */
        return "line: " + text;
    }

    private String used(String text, int line) {
        return text + line;
    }
}
