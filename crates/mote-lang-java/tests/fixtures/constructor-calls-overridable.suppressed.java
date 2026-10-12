// javac reports these constructors under -Xlint:this-escape; the suppression says the author
// looked at the escape and accepted it.
class Panel {
    @SuppressWarnings("this-escape")
    Panel() {
        layout();
    }

    @SuppressWarnings({"unchecked", "this-escape"})
    Panel(int width) {
        layout();
    }

    @SuppressWarnings("unchecked")
    Panel(String title) {
        layout();                                                    // expect: constructor-calls-overridable
    }

    protected void layout() {}
}

@SuppressWarnings(value = "this-escape")
class Dialog {
    Dialog() {
        render();
    }

    protected void render() {}

    class Inner {
        Inner() {
            refresh();
        }

        protected void refresh() {}
    }
}
