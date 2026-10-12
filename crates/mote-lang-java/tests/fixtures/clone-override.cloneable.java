import java.io.Serializable;

class Marker implements Cloneable, Serializable {                    // expect: clone-override
    int value;
}

class QualifiedMarker implements java.lang.Cloneable {             // expect: clone-override
}

class CopiesItself implements Cloneable {
    int value;

    CopiesItself copy() {
        try {
            return (CopiesItself) super.clone();
        } catch (CloneNotSupportedException e) {
            throw new AssertionError(e);
        }
    }
}

class Inherits extends Base implements Cloneable {
}

interface CloneableShape extends Cloneable {
}

class OwnCloneable implements Cloneable2 {
}

interface Cloneable2 {
}

class CopiesThroughThis implements Cloneable {
    CopiesThroughThis copy() {
        try {
            return (CopiesThroughThis) this.clone();
        } catch (CloneNotSupportedException e) {
            throw new IllegalStateException(e);
        }
    }
}
