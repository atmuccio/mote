import java.io.IOException;

// An expectation marker inside a doc comment would describe the tag it sits on, so each
// marker is on the line before the empty tag, on a line that is not a tag's description.
class JavadocEmptyTag {
    /**
     * Moves the cursor. expect-next-line: javadoc-empty-tag
     * @param steps
     * @param <T> the element type. expect-next-line: javadoc-empty-tag
     * @return
     * @throws IOException if it fails. expect-next-line: javadoc-empty-tag
     * @deprecated
     */
    <T> int move(int steps) throws IOException {
        return steps;
    }

    // expect-next-line: javadoc-empty-tag
    /** @return */
    int single() {
        return 0;
    }

    /**
     * Fails. expect-next-line: javadoc-empty-tag
     * @throws IOException
     */
    void fails() throws IOException {
    }

    /**
     * Generic. expect-next-line: javadoc-empty-tag
     * @param <T>
     *
     * @return the value
     */
    <T> T generic() {
        return null;
    }

    /**
     * Moves the cursor.
     *
     * @param steps how far to move
     * @param other
     *        described on the next line
     * @return the new position, or
     *     {@code -1} when closed
     * @throws IOException if the stream fails
     * @exception IllegalStateException {@inheritDoc}
     * @deprecated use {@link #single()} instead
     * @see #single()
     * @since
     * @author
     */
    int documented(int steps, int other) throws IOException {
        return steps;
    }

    /**
     * Example:
     * <pre>{@code
     * @Override
     * @param notATag
     * }</pre>
     * <pre>
     * @return inside pre is not a tag
     * </pre>
     * {@code
     * @throws Nothing
     * }
     *
     * @param value the value
     */
    void example(int value) {
    }

    /*
     * A plain comment.
     * @param x
     */
    void plain(int x) {
    }

    /**
     * Contact me@example.com about @return values.
     */
    void mail() {
    }
}
