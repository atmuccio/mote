// mote-test: params java_release=23
/// A service.
class DeprecatedMarkdown {
    /// Connects.
    ///
    /// @deprecated use [#connect(int)] instead
    public void connect(String host) { }                       // expect: deprecated-annotation-missing

    ///@deprecated no longer used
    int legacyCount;                                           // expect: deprecated-annotation-missing

    /// @deprecated replaced by [Modern]
    static class Old { }                                       // expect: deprecated-annotation-missing

    enum Mode {
        /// @deprecated use B
        A,                                                     // expect: deprecated-annotation-missing
        B
    }

    // Compliant
    /// @deprecated use [#connect(String)]
    @Deprecated
    public void connect(int port) { }

    /// @deprecated use something else
    @java.lang.Deprecated(forRemoval = true)
    void qualified() { }

    /// Mentions @deprecated inside the text, not as a tag.
    void mentioned() { }

    /// Example:
    /// ```java
    /// @deprecated
    /// ```
    void fenced() { }

    /// Lists:
    /// * @deprecated in a list item is not a tag
    void listed() { }

    //// @deprecated in a banner of four slashes
    void banner() { }

    // @deprecated in a plain line comment
    void plain() { }

    /// @deprecated in a run the blank line ends

    /// The nearest run documents the method.
    void separated() { }

    /** Javadoc wins when it is nearest. */
    /// @deprecated in a run followed by a Javadoc comment
    /** Nearest. */
    void javadocNearest() { }

    static class Modern { }
}
