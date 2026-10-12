import java.util.List;

public abstract class Shape {                                       // expect: abstract-class-without-methods
    public abstract double area();

    public abstract String name();
}

abstract class Repository<T> implements AutoCloseable {             // expect: abstract-class-without-methods
    @Override
    public abstract void close();

    public abstract List<T> findAll();

    interface Listener { }
}

abstract class WithState {
    protected int count;

    public abstract void run();
}

abstract class WithProtectedHook {
    public abstract void run();

    protected abstract void hook();
}

abstract class WithConcrete {
    public abstract void run();

    public void helper() { }
}

abstract class WithConstructor {
    WithConstructor() { }

    public abstract void run();
}

abstract class Extending extends Base {
    public abstract void run();
}

abstract class PackageMethod {
    abstract void run();
}

abstract class Framework {
    @Handler
    public abstract void onEvent();
}

class Outer {
    void method() {
        abstract class Local {
            public abstract void run();
        }
    }
}
