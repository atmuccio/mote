import java.io.Externalizable;
import java.io.IOException;
import java.io.ObjectInput;
import java.io.ObjectInputStream;
import java.io.ObjectOutput;
import java.io.ObjectOutputStream;
import java.io.ObjectStreamException;
import java.io.Serializable;

class Settings implements Serializable {
    private static final Settings INSTANCE = new Settings();

    public void readObject(ObjectInputStream in) throws IOException, ClassNotFoundException {   // expect: serialization-hook-signature
        in.defaultReadObject();
    }

    private static void writeObject(ObjectOutputStream out) throws IOException {                // expect: serialization-hook-signature
    }

    private boolean readObjectNoData() {                                                        // expect: serialization-hook-signature
        return true;
    }

    private Settings readResolve() throws ObjectStreamException {                               // expect: serialization-hook-signature
        return INSTANCE;
    }

    static Object writeReplace() {                                                              // expect: serialization-hook-signature
        return INSTANCE;
    }
}

record Point(int x, int y) implements Serializable {
    private void writeObject(ObjectOutputStream out) throws IOException {                       // expect: serialization-hook-signature
        out.defaultWriteObject();
    }

    private Object readResolve() {
        return this;
    }
}

class Good implements Serializable {
    private void writeObject(ObjectOutputStream out) throws IOException {
        out.defaultWriteObject();
    }

    private void readObject(java.io.ObjectInputStream in) throws IOException, ClassNotFoundException {
        in.defaultReadObject();
    }

    protected Object readResolve() {
        return this;
    }

    Object writeReplace() throws ObjectStreamException {
        return this;
    }
}

class NotSerializable {
    public void writeObject(ObjectOutputStream out) throws IOException {
    }

    public Object readObject(ObjectInputStream in) {
        return null;
    }
}

class Helper implements Serializable {
    public void writeObject(ObjectOutputStream out, Object extra) throws IOException {
    }

    public Object readObject() {
        return null;
    }
}

class Ext implements Externalizable {
    public void writeExternal(ObjectOutput out) { }

    public void readExternal(ObjectInput in) { }

    public void writeObject(ObjectOutputStream out) { }
}

enum Color implements Serializable {
    RED;

    private Color readResolve() {
        return RED;
    }
}

class Unknown extends SomeBase {
    public void readObject(ObjectInputStream in) { }
}
