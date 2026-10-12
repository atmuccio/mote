import java.io.Externalizable;
import java.io.IOException;
import java.io.ObjectInput;
import java.io.ObjectOutput;
import java.io.Serializable;

public class Session implements Externalizable {                // expect: serialization-constructor-missing
    private String id;

    Session(String id) { this.id = id; }

    public void writeExternal(ObjectOutput out) throws IOException { out.writeUTF(id); }
    public void readExternal(ObjectInput in) throws IOException { id = in.readUTF(); }

    static class Token implements Externalizable {             // expect: serialization-constructor-missing
        public void writeExternal(ObjectOutput out) {}
        public void readExternal(ObjectInput in) {}
    }

    public static class Ok implements Externalizable {
        public void writeExternal(ObjectOutput out) {}
        public void readExternal(ObjectInput in) {}
    }

    public static class AlsoOk implements Externalizable {
        public AlsoOk() {}
        AlsoOk(int x) {}
        public void writeExternal(ObjectOutput out) {}
        public void readExternal(ObjectInput in) {}
    }

    static class PrivateCtor implements Externalizable {       // expect: serialization-constructor-missing
        private PrivateCtor() {}
        public void writeExternal(ObjectOutput out) {}
        public void readExternal(ObjectInput in) {}
    }

    abstract static class Template implements Externalizable {
        Template(int x) {}
    }

    interface Holder {
        class Inside implements Externalizable {                // implicitly public
            public void writeExternal(ObjectOutput out) {}
            public void readExternal(ObjectInput in) {}
        }
    }
}

class Base {
    protected final int size;

    Base(int size) { this.size = size; }
}

class Child extends Base implements Serializable {            // expect: serialization-constructor-missing
    Child() { super(1); }
}

class Parent2 {
    Parent2() {}
    Parent2(int x) {}
}

class Child2 extends Parent2 implements Serializable {}

class Parent3 implements Serializable {
    Parent3(int x) {}
}

class Child3 extends Parent3 implements Serializable {}       // parent is serializable

class Child4 extends java.util.ArrayList<String> implements Serializable {}

class Parent5 {
    private Parent5() {}
    Parent5(int x) {}
}

class Child5 extends Parent5 implements Serializable {       // expect: serialization-constructor-missing
    Child5() { super(5); }
}
