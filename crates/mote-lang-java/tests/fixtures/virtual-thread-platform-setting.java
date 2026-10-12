class VirtualThreadPlatformSetting {
    void wrong(Runnable task) {
        Thread worker = Thread.ofVirtual().unstarted(task);
        worker.setDaemon(false);                                     // expect: virtual-thread-platform-setting
        worker.setPriority(Thread.MAX_PRIORITY);                     // expect: virtual-thread-platform-setting
        worker.start();

        var named = Thread.ofVirtual().name("io-", 0).unstarted(task);
        named.setPriority(3);                                        // expect: virtual-thread-platform-setting

        Thread.startVirtualThread(task).setPriority(1);              // expect: virtual-thread-platform-setting
        Thread.ofVirtual().start(task).setDaemon(false);             // expect: virtual-thread-platform-setting
    }

    // Compliant
    void fine(Runnable task, Thread given) {
        Thread platform = Thread.ofPlatform().unstarted(task);
        platform.setDaemon(false);
        platform.setPriority(8);

        Thread v = Thread.ofVirtual().unstarted(task);
        v.setDaemon(true);                                           // a no-op, not reported

        Thread maybe = Thread.ofVirtual().unstarted(task);
        maybe = new Thread(task);
        maybe.setPriority(2);                                        // reassigned

        given.setPriority(4);
        new Thread(task).setDaemon(false);
    }
}
