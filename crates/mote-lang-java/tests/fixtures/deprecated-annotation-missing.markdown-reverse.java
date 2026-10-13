// mote-test: params require-javadoc-tag=true java_release=23
class DeprecatedMarkdownReverse {
    /// Connects to the host.
    @Deprecated
    public void connect(String host) { }                       // expect: deprecated-annotation-missing

    /// Connects to the port.
    ///
    /// @deprecated use [#connect(String)]
    @Deprecated
    public void connect(int port) { }

    @Deprecated
    void undocumented() { }
}
