public final class Invoice {
    protected java.math.BigDecimal total;                           // expect: protected-in-final-class
    protected static final int SCALE = 2;                           // expect: protected-in-final-class

    protected Invoice() { }                                         // expect: protected-in-final-class

    protected void recompute() { }                                  // expect: protected-in-final-class

    protected static class Line { }                                 // expect: protected-in-final-class

    @Override
    protected Object clone() {
        return this;
    }

    protected void finalize() { }

    private void helper() { }

    void packagePrivate() { }

    public void api() { }
}

final class Servlet extends HttpServlet {
    protected void doGet(Request req, Response res) { }             // may override without @Override
    protected int counter;                                          // expect: protected-in-final-class
}

class NotFinal {
    protected void hook() { }
}

record Money(long cents) {
    protected static Money zero() {                                 // expect: protected-in-final-class
        return new Money(0);
    }
}

final class Outer {
    class Inner {
        protected void notDirect() { }                              // Inner is not final
    }
}
