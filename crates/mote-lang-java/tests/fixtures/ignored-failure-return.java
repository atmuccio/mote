import java.io.BufferedInputStream;
import java.io.File;
import java.io.FileInputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.Reader;
import java.util.concurrent.locks.Lock;
import java.util.concurrent.locks.ReentrantLock;

class Storage {
    private final Lock lock = new ReentrantLock();
    private final ReentrantLock other = new ReentrantLock();

    void positives(File dir, File tmp, InputStream in, Reader reader, FileInputStream fin)
            throws IOException {
        tmp.delete();                                               // expect: ignored-failure-return
        dir.mkdirs();                                               // "ensure it exists" idiom
        new File("x").createNewFile();                              // expect: ignored-failure-return
        tmp.renameTo(new File(dir, "y"));                           // expect: ignored-failure-return
        tmp.setReadable(true);                                      // expect: ignored-failure-return
        lock.tryLock();                                             // expect: ignored-failure-return
        other.tryLock();                                            // expect: ignored-failure-return
        byte[] buf = new byte[16];
        in.read(buf);                                               // expect: ignored-failure-return
        fin.read(buf, 0, 4);                                        // expect: ignored-failure-return
        new BufferedInputStream(in).read(buf);                      // expect: ignored-failure-return
        reader.read(new char[8]);                                   // expect: ignored-failure-return
        in.skip(10);                                                // expect: ignored-failure-return
    }

    void negatives(File tmp, InputStream in, Repository repo, Unknown u) throws IOException {
        if (!tmp.delete()) {
            throw new IOException("cannot delete " + tmp);
        }
        boolean ok = tmp.mkdirs();
        tmp.deleteOnExit();
        if (lock.tryLock()) {
            lock.unlock();
        }
        in.read();                                                  // single byte, often skipped on purpose
        int n = in.read(new byte[4]);
        repo.delete(tmp);                                           // unknown type
        u.read(new byte[2]);
        in.close();
        delete();
    }

    void delete() { }

    void sameNameTwice(File f) throws IOException {
        try (InputStream s = new FileInputStream(f)) {
            s.read(new byte[4]);                                    // expect: ignored-failure-return
        }
    }

    void otherTypeSameName(StringBuilder s, Holder h) {
        s.append("x");
        h.file().delete();                                          // call result: not checked
    }
}
