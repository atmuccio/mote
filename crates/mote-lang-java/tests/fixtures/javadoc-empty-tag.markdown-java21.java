// mote-test: params java_release=21
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
