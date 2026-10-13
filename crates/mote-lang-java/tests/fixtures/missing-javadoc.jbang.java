///usr/bin/env jbang "$0" "$@" ; exit $?
///DEPS info.picocli:picocli:4.7.0
///SOURCES Helper.java
///JAVA 21+
///PREVIEW

// The shebang line and the directives of the jbang script runner, written with three slashes,
// are build instructions, not documentation.
public class hello {                                          // expect: missing-javadoc
    ///JAVA_OPTIONS -Xmx1g
    public static void main(String... args) {                 // expect: missing-javadoc
    }

    ///Starts the run: documentation that starts right after the slashes.
    public void start() {
    }

    /// DEPS are resolved before the run: a word after a blank is documentation.
    public void resolve() {
    }

    ///MAIN: an upper-case word followed by more than a blank is documentation too.
    public void main2() {
    }
}
