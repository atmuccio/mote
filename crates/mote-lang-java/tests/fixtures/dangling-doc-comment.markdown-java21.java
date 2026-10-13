// mote-test: params java_release=21
public class DanglingMarkdown {
    private int count;

    @Override
    /// After the annotation.
    public String toString() {
        return "x";
    }

    /// Before an initializer block.
    {
        count = 1;
    }

    @Override
    /** A Javadoc comment after the annotation is reported on every release. */  // expect: dangling-doc-comment
    public int hashCode() {
        return 1;
    }

    /// At the end of the body.
}
