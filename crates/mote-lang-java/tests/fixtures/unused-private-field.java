import java.io.Serializable;
import lombok.Getter;

class Fields implements Serializable {
    private static final long serialVersionUID = 1L;
    private int unused;                                     // expect: unused-private-field
    private final String alsoUnused = "x", used = "y";      // expect: unused-private-field
    private int readViaThis;
    private int readInLambda;
    private int readByInner;
    private int[] arr;
    private String reflective;
    @Inject private Object injected;
    @Getter private String generated;
    int packagePrivate;
    protected int prot;
    public int pub;

    int get() {
        Runnable r = () -> System.out.println(readInLambda);
        return this.readViaThis + used.length() + arr.length;
    }

    class Inner {
        int peek() { return Fields.this.readByInner; }
    }

    Object reflect() throws Exception {
        return getClass().getDeclaredField("reflective");
    }

    static class Nested {
        private int nestedUnused;                           // expect: unused-private-field
        private int shadowed;

        void m() {
            int shadowed = 1;   // a same-named local counts as a use (the rule stays silent)
            System.out.println(shadowed);
        }
    }
}

@lombok.Data
class DataClass {
    private String name;
}

enum Status {
    ON, OFF;
    private final int code = 0;                             // expect: unused-private-field
}

record Rec(int a) {
    private static int counter;                             // expect: unused-private-field
}

interface Api {
    int CONSTANT = 1;
}
