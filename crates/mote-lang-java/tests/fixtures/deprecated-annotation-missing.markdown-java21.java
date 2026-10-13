// mote-test: params require-javadoc-tag=true java_release=21
// A `///` run is read only on a release known to be 23 or later.
class DeprecatedMarkdown {
    /// @deprecated use [#connect(int)] instead
    public void connect(String host) { }

    /// Connects to the port.
    @Deprecated
    public void connect(int port) { }

    /** @deprecated Javadoc is read on every release. */
    void old() { }                                             // expect: deprecated-annotation-missing
}
