// A script-style launcher whose class happens to be named like its entry point.
public class main {
    public static void main(String... args) {
        System.out.println("hello");
    }
}

class Builder {
    // A static factory named like the class: not a mistyped constructor.
    public static Builder Builder() {
        return new Builder();
    }

    // An instance method named like the class still looks like a constructor.
    public void Builder(int size) {                                // expect: confusing-member-name
    }
}
