// mote-test: params java_release=23
// mote-test: path src/main/java/demo/Ledger.java
package demo;

import java.util.List;

/// A ledger of entries, documented with a Markdown documentation comment.
///
/// See [List] for the entries.
public final class Ledger {
    /// The largest number of entries.
    public static final int MAX = 100;

    ///No space after the slashes is still documentation.
    public static final int MIN = 1;

    //// A banner of four slashes is not documentation.
    public static final int LIMIT = 10;                       // expect: missing-javadoc

    // A plain line comment is not documentation.
    public int size;                                          // expect: missing-javadoc

    /// Creates an empty ledger.
    public Ledger() {
    }

    /// Adds an entry.
    ///
    /// @param amount the amount to add
    /// @return the new balance
    public long add(long amount) {
        return amount;
    }

    /// Documented, then annotated.
    @Deprecated
    public void old() {
    }

    @Deprecated
    /// After the annotation: counts here (dangling-doc-comment reports its position).
    public void legacy() {
    }

    /// A line comment between the doc comment and the method does not hide it.
    // note
    public void noted() {
    }

    /// A blank line between the doc comment and the method does not detach it.

    public void spaced() {
    }

    /// The comment nearest the method documents it; the run above is a separate one.

    /// Second run.
    public void twoRuns() {
    }

    int count; /// a trailing comment documents the field before, not the next method
    public void afterTrailing() {                             // expect: missing-javadoc
    }

    /*/ A block comment that starts like a slash run. */
    public void plainBlock() {                                // expect: missing-javadoc
    }

    /// Documents the nested interface.
    public interface Listener {
        /// Called on change.
        void changed();

        void undocumented();                                  // expect: missing-javadoc
    }

    /// Documents the enum.
    public enum Kind {
        DEBIT,
        CREDIT;

        /// The sign of the kind.
        public int sign() {
            return this == DEBIT ? -1 : 1;
        }

        public int weight() {                                 // expect: missing-javadoc
            return 1;
        }
    }

    /// Documents the record.
    public record Entry(long amount) {
        /// Whether the entry is empty.
        public boolean isZero() {
            return amount == 0;
        }
    }

    public void undocumented() {                              // expect: missing-javadoc
    }
}
