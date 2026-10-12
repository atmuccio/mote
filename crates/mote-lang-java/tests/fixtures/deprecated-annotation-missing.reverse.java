// mote-test: params require-javadoc-tag=true
class DeprecatedReverse {
    /**
     * No tag here.
     */
    @Deprecated
    void annotatedOnly() { }                                   // expect: deprecated-annotation-missing

    /**
     * @deprecated use {@link #other()}
     */
    void tagOnly() { }                                         // expect: deprecated-annotation-missing

    // Compliant
    /**
     * @deprecated use {@link #other()}
     */
    @Deprecated
    void both() { }

    @Deprecated
    void noJavadoc() { }

    void other() { }
}
