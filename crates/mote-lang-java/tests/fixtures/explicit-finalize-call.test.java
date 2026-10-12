// mote-test: test-file
class ExplicitFinalizeCallTest {
    void finalizerReleasesTheHandle(Object handle) throws Throwable {
        java.lang.reflect.Method m = null;
        ((Resource) handle).finalize();                             // a test exercising a finalizer
    }

    static class Resource {
        @Override
        public void finalize() { }
    }
}
