// The file declares its own annotation type named `Override`: `@Override` would mean that one,
// not java.lang.Override, so the finding is reported without a fix.
@interface Override {
}

class Job implements Runnable {
    public void run() {                                   // expect: missing-override
    }
}
