// No Java release is known: a `///` run may be an ordinary comment, so it is not read.
import java.io.IOException;

class JavadocEmptyTagMarkdown {
    /// Moves the cursor.
    /// @param steps
    /// @return
    /// @throws IOException
    /// @deprecated
    int move(int steps) throws IOException {
        return steps;
    }

    /**
     * Javadoc is read on every release. expect-next-line: javadoc-empty-tag
     * @return
     */
    int single() {
        return 0;
    }
}
