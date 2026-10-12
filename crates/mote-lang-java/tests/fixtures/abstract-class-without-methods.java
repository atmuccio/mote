import java.io.Serializable;

public abstract class Defaults {                                    // expect: abstract-class-without-methods
    public static final int TIMEOUT = 30;
}

abstract class Empty {                                              // expect: abstract-class-without-methods
}

abstract class DataHolder {                                         // expect: abstract-class-without-methods
    protected String id;
    protected long created;

    static class Nested { }
}

abstract class WithMethod {
    abstract void run();
}

abstract class WithConcreteMethod {
    void run() { }
}

abstract class WithConstructor {
    protected WithConstructor(String id) { }
}

abstract class Extending extends Base {
}

abstract class Implementing implements Serializable {
}

abstract class TypeToken<T> {
}

sealed abstract class Shape permits Circle, Square {
}

@MappedSuperclass
abstract class Persisted {
    protected long id;
}

class Concrete {
}

interface Api {
}
