import java.io.File;
import java.io.IOException;

class Installer {
    static class Download {
        protected File target;
        protected int attempts;

        protected synchronized void replace(File dst, File src) throws IOException {
            target = dst;
        }

        protected synchronized void finish(File dst) {
            target = dst;
        }

        protected synchronized void retry(File dst) {
            attempts++;
        }

        protected synchronized void swap(File dst) {
            target = dst;
        }
    }

    static class VerifiedDownload extends Download {
        private final String expectedDigest = "abc";

        // A read-only check, then all the state-changing work under the same lock.
        @Override
        protected void replace(File dst, File src) throws IOException {
            if (src.length() == 0) {
                verify(src);
            }
            String name = dst.getName();
            synchronized (this) {
                File backup = new File(name + ".bak");
                target = backup;
                target = dst;
            }
        }

        // Writes a field before taking the lock.
        @Override
        protected void finish(File dst) {                            // expect: override-drops-synchronized
            attempts = 0;
            synchronized (this) {
                target = dst;
            }
        }

        // Work after the locked block runs without the lock.
        @Override
        protected void retry(File dst) {                             // expect: override-drops-synchronized
            synchronized (this) {
                attempts++;
            }
            verify(dst);
        }

        // Locks another object.
        @Override
        protected void swap(File dst) {                              // expect: override-drops-synchronized
            verify(dst);
            synchronized (expectedDigest) {
                target = dst;
            }
        }

        void verify(File f) { }
    }
}
