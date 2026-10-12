/**
 * A licence header written as a doc comment, before the package line: not reported.
 */
package demo;

/** Before an import: not reported either. */
import java.util.List;

/** Documents the class. */
public class DanglingDocComment {
    /** Documents the field. */
    private int count;

    @Override
    /** After the annotation: ignored by the documentation tool. */         // expect: dangling-doc-comment
    public String toString() {
        return "x";
    }

    @Deprecated
    @SuppressWarnings("unused")
    /**                                                                     // expect: dangling-doc-comment
     * Also after the annotations.
     */
    void old() {
    }

    /** Before the annotations: fine. */
    @Deprecated
    void documented() {
        /** A note in a body. */                                            // expect: dangling-doc-comment
        int attempts = 3;
        /** Before a statement. */                                          // expect: dangling-doc-comment
        count += attempts;
        /** Documents a local class: accepted. */
        class Local {
        }
        /* A plain comment. */
        // A line comment.
        /*****************************************
         * A banner comment, not documentation.
         *****************************************/
        count++;
        /**/
        Runnable r = () -> {
            /** Inside a lambda body. */                                    // expect: dangling-doc-comment
            count--;
        };
        foo(/** inline */ 1);
    }

    /** Before an initializer block. */                                     // expect: dangling-doc-comment
    static {
        System.out.println();
    }

    /** Documents the method, with a line comment in between. */
    // note
    void foo(int x) {
    }

    enum Mode {
        /** Documents a constant. */
        A,
        B;

        /** Documents an enum method. */
        void m() {
        }
    }

    interface Api {
        /** Documents an interface method. */
        void run();
    }

    record Point(int x, int y) {
        /** Documents a compact constructor. */
        Point {
        }
    }

    /** Documents nothing: the class body ends here. */                    // expect: dangling-doc-comment
}
