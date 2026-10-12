import java.io.FilterInputStream;
import java.io.FilterOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;

class CountingInputStream extends FilterInputStream {                  // expect: stream-missing-bulk-method
    long count;

    CountingInputStream(InputStream in) {
        super(in);
    }

    @Override
    public int read() throws IOException {
        int b = super.read();
        if (b >= 0) {
            count++;
        }
        return b;
    }
}

class ZeroStream extends InputStream {                                  // expect: stream-missing-bulk-method
    @Override
    public int read() {
        return 0;
    }

    @Override
    public int read(byte[] b) throws IOException {
        return read(b, 0, b.length);
    }
}

class NullSink extends OutputStream {                                  // expect: stream-missing-bulk-method
    @Override
    public void write(int b) {
    }
}

class Complete extends InputStream {
    @Override
    public int read() {
        return -1;
    }

    @Override
    public int read(byte[] b, int off, int len) {
        return -1;
    }
}

class CompleteOut extends FilterOutputStream {
    CompleteOut(OutputStream out) {
        super(out);
    }

    @Override
    public void write(int b) throws IOException {
        out.write(b);
    }

    @Override
    public void write(byte[] b, int off, int len) throws IOException {
        out.write(b, off, len);
    }
}

abstract class Base extends InputStream {
    @Override
    public int read() {
        return -1;
    }
}

class Other extends java.io.Reader {
    public int read(char[] c, int o, int l) {
        return -1;
    }

    public void close() {
    }
}

class Anonymous {
    OutputStream sink() {
        return new OutputStream() {                                     // expect: stream-missing-bulk-method
            @Override
            public void write(int b) {
            }
        };
    }
}
