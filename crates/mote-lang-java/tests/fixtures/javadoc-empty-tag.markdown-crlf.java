// mote-test: params java_release=23
import java.io.IOException;

// An expectation marker inside a doc comment would describe the tag it sits on, so each
// marker is on the line before the empty tag, on a line that is not a tag's description.
class JavadocEmptyTagMarkdown {
    /// Moves the cursor. expect-next-line: javadoc-empty-tag
    /// @param steps
    /// @param <T> the element type. expect-next-line: javadoc-empty-tag
    /// @return
    /// @throws IOException if it fails. expect-next-line: javadoc-empty-tag
    /// @deprecated
    <T> int move(int steps) throws IOException {
        return steps;
    }

    // expect-next-line: javadoc-empty-tag
    /// @return
    int single() {
        return 0;
    }

    /// Reads.
    ///
    /// @param count how many
    ///     items to read, continued on the next line
    /// @return
    ///     the number read, described on the next line
    int read(int count) {
        return count;
    }

    /// Example:
    /// ```java
    /// @param x
    /// @return
    /// ```
    /// ~~~
    /// @throws IOException
    /// ~~~
    /// @param value the value
    void fenced(int value) {
    }

    /// Lists:
    /// * @param not a tag but a list item
    /// - @return neither
    /// @param value the value, with {@code @return} inside an inline tag
    void lists(int value) {
    }

    //// @param value: four slashes, an ordinary comment
    //// @return
    void banner(int value) {
    }

    // @return: a plain line comment
    int plain() {
        return 0;
    }

    /// Two runs: the blank line ends the first one. expect-next-line: javadoc-empty-tag
    /// @param value

    /// @return the value
    int split(int value) {
        return value;
    }

    int field; /// @return: a trailing comment, not documentation
}
