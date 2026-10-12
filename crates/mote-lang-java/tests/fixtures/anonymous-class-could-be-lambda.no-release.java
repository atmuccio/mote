// Without a known Java release the rule stays silent.
class Legacy {
    void start(java.util.concurrent.Executor executor) {
        executor.execute(new Runnable() {
            @Override
            public void run() {
                System.out.println("x");
            }
        });
    }
}
