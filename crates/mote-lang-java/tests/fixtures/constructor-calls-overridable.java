class Widget {
    private final String name;

    Widget(String name) {
        this.name = name;
        layout();                                                    // expect: constructor-calls-overridable
        this.refresh(1);                                             // expect: constructor-calls-overridable
        init();
        setup();
        helper();
        layout();
        Runnable r = () -> layout();
        other.layout();
    }

    protected void layout() {}

    public void refresh(int times) {}

    private void init() {}

    final void setup() {}

    static void helper() {}

    Widget other;
}

abstract class Template {
    Template() {
        configure();                                                 // expect: constructor-calls-overridable
    }

    abstract void configure();
}

final class Sealed {
    Sealed() {
        layout();
    }

    void layout() {}
}

class OnlyPrivate {
    private OnlyPrivate() {
        layout();
    }

    void layout() {}

    static OnlyPrivate create() {
        return new OnlyPrivate();
    }
}

enum Mode {
    A;

    Mode() {
        describe();
    }

    void describe() {}
}

class Inherited extends Widget {
    Inherited() {
        super("x");
        inheritedOnly();
    }
}

class Holder {
    private static class Hidden {
        Hidden() {
            layout();
        }

        void layout() {}
    }

    private static class Extended {
        Extended() {
            layout();                                                // expect: constructor-calls-overridable
        }

        void layout() {}
    }

    private static class Sub extends Extended {}
}
