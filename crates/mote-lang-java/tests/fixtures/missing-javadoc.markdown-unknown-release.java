// mote-test: path src/main/java/demo/Ledger.java
// No Java release is known: a `///` run is documentation all the same.
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

    int count; /// trailing
    public void afterTrailing() {                             // expect: missing-javadoc
    }
}
