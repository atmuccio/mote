import lombok.experimental.UtilityClass;

public class Strings {                                   // expect: utility-class-constructor
    public static final String EMPTY = "";
    public static boolean isBlank(String s) { return s == null || s.isBlank(); }
}

final class WithPrivateCtor {
    private WithPrivateCtor() {}
    static int twice(int x) { return 2 * x; }
}

class WithPublicCtor {
    public WithPublicCtor() {}                           // explicitly instantiable: not reported
    static int twice(int x) { return 2 * x; }
}

class OnlyConstants {
    static final int A = 1;                              // no static method: not reported
}

class HasInstanceMethod {
    static int a() { return 1; }
    int b() { return 2; }
}

class HasInstanceField {
    int state;
    static int a() { return 1; }
}

abstract class AbstractHelpers {
    static int a() { return 1; }
}

class Extends extends Object {
    static int a() { return 1; }
}

class Implements implements Runnable {
    public static void helper() {}
    public void run() {}
}

class Launcher {
    public static void main(String[] args) {}
    static void help() {}
}

@UtilityClass
class LombokUtils {
    static int a() { return 1; }
}

@org.springframework.context.annotation.Configuration
class SpringConfig {
    static Object bean() { return null; }
}

class WithInstanceInit {
    { System.out.println("init"); }
    static void a() {}
}

class Holder {
    static class NestedUtils {                           // expect: utility-class-constructor
        static void a() {}
    }
    private static class PrivateNested {
        static void a() {}
    }
    class InnerNotStatic {
        static void a() {}
    }
    void m() {
        class Local {
            static void a() {}
        }
    }
    int x;
}

class StaticInitAndNestedTypes {                         // expect: utility-class-constructor
    static { System.loadLibrary("x"); }
    interface Callback { void call(); }
    enum Mode { A }
    static void run(Callback c) { c.call(); }
}

class HasInnerClass {
    class Inner {}
    static void a() {}
}

enum NotAClass {
    A;
    static void a() {}
}

interface NotAClassEither {
    static void a() {}
}
