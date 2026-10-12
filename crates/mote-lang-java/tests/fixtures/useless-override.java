import java.util.ArrayList;
import java.util.List;

class Base {
    protected void start(String context) { }
    void stop() { }
    protected Object value(int a, int b) { return null; }
    protected void hidden() { }
}

class Child extends Base {
    @Override // expect-next-line: useless-override
    protected void start(String context) {
        super.start(context);
    }

    @Override // expect-next-line: useless-override
    void stop() {
        super.stop();
    }

    @Override // expect-next-line: useless-override
    protected Object value(int a, int b) {
        return super.value(a, b);
    }

    // Wider than the parent in this file: deliberate.
    @Override
    public void hidden() {
        super.hidden();
    }
}

class Fine extends Base {
    // Adds behavior.
    @Override
    protected void start(String context) {
        super.start(context);
        System.out.println(context);
    }

    // Different arguments.
    @Override
    protected Object value(int a, int b) {
        return super.value(b, a);
    }

    // Another annotation.
    @Override
    @Deprecated
    void stop() {
        super.stop();
    }

    /** Documents the inherited behavior for this class. */
    @Override
    protected void hidden() {
        super.hidden();
    }
}

class NoOverrideAnnotation extends ArrayList<String> {
    public boolean add(String s) {
        return super.add(s);
    }
}

class Modifiers extends Base {
    @Override
    protected final void start(String context) {
        super.start(context);
    }

    @Override
    synchronized void stop() {
        super.stop();
    }
}

class Cloning implements Cloneable {
    @Override
    public Object clone() throws CloneNotSupportedException {
        return super.clone();
    }

    @Override
    protected void finalize() throws Throwable {
        super.finalize();
    }
}

@lombok.Data
class Entity {
    private String name;

    @Override
    public String toString() {
        return super.toString();
    }
}

class Plain {
    @Override // expect-next-line: useless-override
    public String toString() {
        return super.toString();
    }

    @Override // expect-next-line: useless-override
    public boolean equals(Object o) {
        return super.equals(o);
    }
}

class Params extends Base {
    @Override
    protected void start(@Nullable String context) {
        super.start(context);
    }
}

class Anonymous {
    Runnable r = new Thread() {
        @Override
        public void run() {
            super.run();
        }
    };
}

class Varargs extends Parent {
    @Override
    public void log(String fmt, Object... args) {
        super.log(fmt, args);
    }
}

class Exposed extends java.net.URLClassLoader {
    Exposed() { super(new java.net.URL[0]); }

    // Make the method visible.
    @Override
    public void addURL(java.net.URL url) {
        super.addURL(url);
    }
}

class Commented extends Base {
    /*
     * Needed to keep a static analyzer quiet.
     */
    @Override
    void stop() {
        super.stop();
    }

    @Override
    protected void start(String context) {
        // Nothing to add yet; kept as an extension point.
        super.start(context);
    }

    @Override // expect-next-line: useless-override
    protected Object value(int a, int b) {
        // TODO Auto-generated method stub
        return super.value(a, b);
    }
}

class PublicParent {
    public void open() { }
}

class PublicChild extends PublicParent {
    @Override // expect-next-line: useless-override
    public void open() {
        super.open();
    }
}

class TestLike extends junit.framework.TestCase {
    @Override // expect-next-line: useless-override
    protected void tearDown() throws Exception {
        super.tearDown();
    }

    @Override
    public void setUp() throws Exception {
        super.setUp();
    }
}

class OtherPackageParent extends org.acme.base.Connection {
    // Re-declared protected: opens the method to this package.
    @Override
    protected void handleError(Throwable t) {
        super.handleError(t);
    }
}

class SamePackageParent extends Connection {
    @Override // expect-next-line: useless-override
    protected void handleError(Throwable t) {
        super.handleError(t);
    }

    @Override // expect-next-line: useless-override
    void close() {
        super.close();
    }
}
