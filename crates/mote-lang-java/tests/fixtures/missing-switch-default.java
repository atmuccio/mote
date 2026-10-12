class Switches {
    enum Color { RED, GREEN }
    static final int OK = 200;

    void positives(int code, String key, char c) {
        switch (code) { // expect: missing-switch-default
            case 200: ok(); break;
            case 404: missing(); break;
        }
        switch (key) { // expect: missing-switch-default
            case "a":
            case "b":
                ok();
                break;
        }
        for (int i = 0; i < 3; i++)
            switch (c) { // expect: missing-switch-default
                case 'x': ok();
            }
        label:
        switch (code) { // expect: missing-switch-default
            case 1: break label;
        }
    }

    int negatives(int code, Color color, Object o, String key) {
        switch (code) {
            case 200: ok(); break;
            default: missing();
        }
        switch (color) {
            case RED: ok(); break;
            case GREEN: missing(); break;
        }
        switch (code) { // expect: missing-switch-default
            case OK: ok(); break;           // an int selector: OK is an int constant of this file
        }
        switch (o) {
            case String s: ok(); break;
            case Integer i: missing(); break;
            default: break;
        }
        int v = switch (code) {
            case 1: yield 10;
            default: yield 20;
        };
        switch (key) {
            case "a": ok(); break;
            case null, default: missing();
        }
        switch (code) {
        }
        return v;
    }

    void arrows(int code, String key, Color color, Mode mode) {
        switch (code) { // expect: missing-switch-default
            case 1 -> ok();
            case 2 -> missing();
        }
        switch (key) { // expect: missing-switch-default
            case "a", "b" -> ok();
        }
        switch (code) {
            case 1 -> ok();
            default -> missing();
        }
        String s = switch (color) {
            case RED -> "r";
            case GREEN -> "g";
        };
        switch (mode) {
            case ON -> ok();                 // enum declared elsewhere: its constants are unknown
        }
    }

    void enums(Color color, Level level, Outer.Inner inner) {
        switch (level) { // expect: missing-switch-default
            case LOW: ok(); break;
            case HIGH: missing(); break;
        }
        switch (level) { // expect: missing-switch-default
            case LOW -> ok();
        }
        switch (level) {
            case LOW, MEDIUM -> ok();
            case HIGH -> missing();
        }
        switch (level) {
            case LOW:
            case MEDIUM:
                ok();
                break;
            case HIGH:
                break;
        }
        switch (inner) { // expect: missing-switch-default
            case A: ok(); break;
        }
        switch (level) {
            case LOW: ok(); break;
            default: break;
        }
        switch (this.level) { // expect: missing-switch-default
            case HIGH: ok(); break;
        }
    }

    Level level;

    enum Level {
        LOW, MEDIUM, HIGH;
        int weight() { return ordinal(); }
    }

    static class Outer {
        enum Inner { A, B }
    }

    void ok() {}
    void missing() {}
}

class QualifiedConstants {
    static final class Codes { static final int A = 1; static final int B = 2; }

    void run(int code) {
        switch (code) { // expect: missing-switch-default
            case Codes.A:
            case Codes.B:
                break;
        }
    }
}
