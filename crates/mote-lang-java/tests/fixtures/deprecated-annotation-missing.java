/**
 * A service.
 */
class DeprecatedAnnotationMissing {
    /**
     * Connects.
     *
     * @deprecated use {@link #connect(int)} instead
     */
    public void connect(String host) { }                       // expect: deprecated-annotation-missing

    /** @deprecated no longer used */
    int legacyCount;                                           // expect: deprecated-annotation-missing

    /**
     * @deprecated
     */
    DeprecatedAnnotationMissing() { }                          // expect: deprecated-annotation-missing

    /**
     * @deprecated replaced by {@link Modern}
     */
    static class Old { }                                       // expect: deprecated-annotation-missing

    enum Mode {
        /** @deprecated use B */
        A,                                                     // expect: deprecated-annotation-missing
        B
    }

    interface Api {
        /** @deprecated */
        int VERSION = 1;                                       // expect: deprecated-annotation-missing
    }

    // Compliant
    /**
     * @deprecated use {@link #connect(String)}
     */
    @Deprecated
    public void connect(int port) { }

    /**
     * @deprecated since 2.0
     */
    @java.lang.Deprecated(since = "2.0", forRemoval = true)
    void old() { }

    /**
     * Mentions @deprecated in the text, which is not a tag.
     * Also {@code @deprecated} inline.
     */
    void mention() { }

    /**
     * Returns the deprecatedValue.
     * @deprecatedValue not a tag either
     */
    void notATag() { }

    /* @deprecated in a plain comment */
    void plainComment() { }

    /**
     * No tag here.
     */
    @Deprecated
    void annotatedOnly() { }                                   // reported only with require-javadoc-tag

    @Deprecated
    void noJavadoc() { }

    static class Modern { }
}
