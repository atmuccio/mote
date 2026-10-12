import java.util.function.Supplier;

class Brackets {
    private char open = '[';

    String render(char sep, int capacity, Object o) {
        StringBuilder a = new StringBuilder('[');                 // expect: string-builder-char-constructor
        StringBuffer b = new StringBuffer('\n');                  // expect: string-builder-char-constructor
        StringBuilder c = new StringBuilder(sep);                 // expect: string-builder-char-constructor
        StringBuilder d = new java.lang.StringBuilder(open);      // expect: string-builder-char-constructor
        StringBuilder e = new StringBuilder(('x'));               // expect: string-builder-char-constructor
        Supplier<StringBuilder> f = () -> new StringBuilder('(');  // expect: string-builder-char-constructor

        // Compliant
        StringBuilder g = new StringBuilder("[");
        StringBuilder h = new StringBuilder(capacity);
        StringBuilder i = new StringBuilder(16);
        StringBuilder j = new StringBuilder().append('[');
        StringBuilder k = new StringBuilder(String.valueOf(sep));
        StringBuilder l = new StringBuilder((int) sep);
        StringBuilder m = new StringBuilder(o.toString());
        return a + "" + b + c + d + e + f + g + h + i + j + k + l + m;
    }
}

class OwnBuilder {
    static class StringBuilder {
        StringBuilder(char c) { }
    }

    void shadowed() {
        StringBuilder s = new StringBuilder('x');                 // a local class, not java.lang
    }
}
