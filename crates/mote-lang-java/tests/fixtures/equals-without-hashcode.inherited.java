import java.util.Objects;

class Identity {
    @Override
    public boolean equals(Object other) {
        return this == other;
    }
}

class IdentityReversed {
    @Override
    public boolean equals(final Object obj) {
        return (obj == this);
    }
}

class NotIdentity {
    Object peer;

    @Override
    public boolean equals(Object other) {                           // expect: equals-without-hashcode
        return peer == other;
    }
}

abstract class Entry {
    @Override
    public abstract boolean equals(Object o);

    @Override
    public abstract int hashCode();
}

// The abstract hashCode() above forces every concrete subclass to write one.
abstract class Ref extends Entry {
    int id;

    @Override
    public boolean equals(Object o) {
        return o instanceof Ref && ((Ref) o).id == id;
    }
}

class MethodRef extends Ref {
    private int cached;

    // equals is inherited from Ref and compares the same field.
    @Override
    public int hashCode() {
        if (cached == 0) {
            cached = Objects.hash(id);
        }
        return cached;
    }
}

// A superclass of another file may redeclare hashCode() abstract as well.
abstract class ExternalRef extends ExternalEntry {
    int id;

    @Override
    public boolean equals(Object o) {
        return o instanceof ExternalRef && ((ExternalRef) o).id == id;
    }
}

// Concrete: the superclass of another file cannot excuse it.
class ConcreteExternal extends ExternalEntry {
    int id;

    @Override
    public boolean equals(Object o) {                               // expect: equals-without-hashcode
        return o instanceof ConcreteExternal && ((ConcreteExternal) o).id == id;
    }
}

// hashCode only, equals possibly inherited from a class of another file.
class CachedHash extends ExternalValue {
    @Override
    public int hashCode() {
        return 7;
    }
}

class Plain {
    int x;
}

// Every superclass is in this file and none declares equals: Object's identity equals remains.
class PlainHash extends Plain {
    @Override
    public int hashCode() {                                         // expect: equals-without-hashcode
        return x;
    }
}

class Generic<T> {
    T value;
}

class GenericHash extends Generic<String> {
    @Override
    public int hashCode() {                                         // expect: equals-without-hashcode
        return 1;
    }
}
