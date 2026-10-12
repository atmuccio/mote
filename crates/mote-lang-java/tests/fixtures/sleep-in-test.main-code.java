class Poller {
    void backoff() throws InterruptedException {
        Thread.sleep(100);                                          // production code: not checked
    }
}
