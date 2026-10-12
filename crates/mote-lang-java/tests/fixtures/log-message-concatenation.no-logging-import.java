class NoImports extends UnknownBase {
    void m(String s) {
        log.debug("value " + s);                                    // unknown `log`: no logging API imported
    }
}
