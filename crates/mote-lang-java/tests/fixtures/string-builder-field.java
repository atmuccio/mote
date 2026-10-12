class ReportWriter {
    private final StringBuilder buffer = new StringBuilder();       // expect: string-builder-field
    private StringBuffer legacy;                                    // expect: string-builder-field
    private static java.lang.StringBuilder shared;                  // expect: string-builder-field
    private String text;
    private StringBuilder[] many;                                   // arrays are not reported
    private com.acme.StringBuilder custom;                          // not java.lang

    String render() {
        StringBuilder local = new StringBuilder();
        return local.toString();
    }
}
