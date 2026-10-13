// mote-test: params java_release=23
///usr/bin/env jbang "$0" "$@" ; exit $?
//DEPS com.example:lib:1.0

/// Before the package line: a file header, not reported.
package demo;

/// Before an import: not reported either.
import java.util.List;

/// Documents the class.
public class DanglingMarkdown {
    /// Documents the field.
    private int count;

    @Override
    /// After the annotation: ignored by the documentation tool.     expect: dangling-doc-comment
    /// The whole run is one comment, reported once.
    public String toString() {
        return "x";
    }

    @Deprecated
    @SuppressWarnings("unused")
    /// Also after the annotations.                                  expect: dangling-doc-comment
    void old() {
    }

    @Override /// A trailing comment after the annotation is not documentation.
    public int hashCode() {
        return 1;
    }

    /// Before the annotations: fine.
    @Deprecated
    void documented() {
        /// A note in a body: `///` there is often an ordinary note, not reported.
        int attempts = 3;
        ///count += attempts;
        count += attempts;
    }

    /// A blank line before the declaration does not detach the comment.

    void spaced() {
    }

    //// A banner of four slashes.
    //// Not documentation.
    static {
    }

    /// Before an initializer block: documents nothing.              expect: dangling-doc-comment
    {
        count = 1;
    }

    /// At the end of the body: documents nothing.                    expect: dangling-doc-comment
}

/// Documents the enum.
enum Mode {
    /// Documents the constant.
    A,
    B
}
