class Blocks {
    int counter;
    java.util.List<String> cache, listeners;

    void reset() {
        counter = 0;
        {                                                          // expect: lone-block
            cache.clear();
            listeners.clear();
        }
    }

    Blocks() {
        {                                                          // expect: lone-block
            counter = 1;
        }
    }

    void nested() {
        if (counter > 0) {
            {                                                      // expect: lone-block
                counter--;
            }
        }
        Runnable r = () -> {
            {                                                      // expect: lone-block
                counter++;
            }
        };
    }

    void scoped() {
        {
            int tmp = counter;
            counter = tmp + 1;
        }
        {
            class Local { }
            new Local();
        }
        {
        }
        label: {
            if (counter > 3) break label;
            counter++;
        }
        switch (counter) {
            case 1: {
                counter++;
                break;
            }
            default:
                break;
        }
        switch (counter) {
            case 2 -> {
                counter--;
            }
            default -> counter = 0;
        }
        synchronized (this) {
            counter++;
        }
    }

    {
        counter = 5;
    }

    static {
        System.setProperty("a", "b");
    }
}
