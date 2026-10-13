// mote-test: params java_release=21
// mote-test: path src/main/java/demo/Ledger.java
// A project compiled for Java 21 may be documented with `///` runs (the JDK running javadoc
// decides): they count on any release.
package demo;

/// A ledger.
public final class Ledger {
    /// The largest number of entries.
    public static final int MAX = 100;

    /// Adds an entry.
    ///
    /// @param amount the amount to add
    public void add(long amount) {
    }

    @Deprecated
    /// After the annotation.
    public void legacy() {
    }

    //// A banner of four slashes is not documentation.
    public void banner() {                                    // expect: missing-javadoc
    }

    /** A Javadoc comment documents on every release. */
    public void documented() {
    }

    public void undocumented() {                              // expect: missing-javadoc
    }
}
