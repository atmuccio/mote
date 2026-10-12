class PriceView implements PriceListener {
    @SuppressWarnings("this-escape")
    PriceView(PriceFeed feed) {
        feed.addListener(this);
        init();
    }

    PriceView(PriceFeed feed, int scale) {
        feed.addListener(this);                                      // expect: this-escape-in-constructor
        init();
    }

    void init() {}
}

@SuppressWarnings({"this-escape"})
class Poller {
    Poller(Registry registry) {
        registry.register(this);
        start();
    }

    void start() {}
}
