// mote-test: path src/main/java/demo/Ledger.java
// A doc comment documents the next declaration of its body, whatever comes in between: other
// comments and stray semicolons are skipped, anything else takes the comment.
package demo;

/** Documented. */
public class Ledger {
    /** First comment. */
    /** Second comment: still documents the next method. */
    public void twice() {
    }

    /** Documented across a stray semicolon. */
    ;
    public void afterSemicolon() {
    }

    /** Taken by the initializer below. */
    {
        System.out.println("init");
    }
    public void afterInitializer() {                          // expect: missing-javadoc
    }

    /** Taken by the static initializer below. */
    static {
        System.out.println("static init");
    }
    public void afterStaticInitializer() {                    // expect: missing-javadoc
    }

    /** Taken by the private field below. */
    private int hidden;
    public int notDocumented() {                              // expect: missing-javadoc
        return hidden;
    }

    /** Documented nested class. */
    public static class Entry {
        /** Documented. */
        public void post() {
        }

        public void undo() {                                  // expect: missing-javadoc
        }

        /** Dangling at the end of the nested body. */
    }
    public void afterNested() {                               // expect: missing-javadoc
    }

    /** Documented enum. */
    public enum Side {
        DEBIT, CREDIT /** Before the semicolon: in the constants, not before a member. */;

        public int sign() {                                   // expect: missing-javadoc
            return 1;
        }
    }

    public Runnable task() {                                  // expect: missing-javadoc
        return new Runnable() {
            /** Inside an anonymous class. */
            public void run() {
            }
        };
    }
    public void afterAnonymous() {                            // expect: missing-javadoc
    }
}
