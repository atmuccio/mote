import java.util.function.Consumer;

class PriceView implements Runnable {
    private final Thread worker;
    private String format;

    PriceView(Feed feed, Bus bus) {
        feed.addListener(this);                                      // expect: this-escape-in-constructor
        bus.register(this);                                          // expect: this-escape-in-constructor
        worker = new Thread(this);
        worker.start();                                              // expect: this-escape-in-constructor
        new Thread(() -> run()).start();                             // expect: this-escape-in-constructor
        format = "0.00";
        this.addListener(this);
        feed.addListener(new Other());
        Runnable later = () -> feed.addListener(this);
        feed.setOwner(this);
    }

    void addListener(Object o) {}

    public void run() {}

    interface Feed {
        void addListener(Object l);
        void setOwner(Object o);
    }

    interface Bus {
        void register(Object o);
    }

    static class Other {}
}

final class LastStatement {
    LastStatement(PriceView.Feed feed) {
        init();
        feed.addListener(this);
    }

    void init() {}
}

final class NotLast {
    NotLast(PriceView.Feed feed) {
        feed.addListener(this);                                      // expect: this-escape-in-constructor
        init();
    }

    void init() {}
}

class NotAThread {
    NotAThread(Engine e) {
        e.start();
    }

    interface Engine {
        void start();
    }
}

class OwnComponents implements java.awt.event.ActionListener {
    private final javax.swing.JButton ok = new javax.swing.JButton("OK");
    private javax.swing.JButton cancel;

    OwnComponents() {
        ok.addActionListener(this);
        cancel = new javax.swing.JButton("Cancel");
        cancel.addActionListener(this);
        javax.swing.JButton local = new javax.swing.JButton("x");
        local.addActionListener(this);
    }

    public void actionPerformed(java.awt.event.ActionEvent e) {}
}
