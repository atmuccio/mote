import java.io.InputStream;

class Frames {
    int wrong(byte[] header, byte hi, byte lo, byte b0, byte b1, byte b2, byte b3) {
        int length = (header[0] << 8) | header[1];                    // expect: byte-sign-extension
        int word = (hi << 8) + lo;                                    // expect: byte-sign-extension
        int all = (b0 << 24) | (b1 << 16) | (b2 << 8) | b3;           // expect: byte-sign-extension
        int swapped = header[3] | (header[2] << 8);                   // expect: byte-sign-extension
        return length + word + all + swapped;
    }

    int fine(byte[] header, byte flags, byte mask, int x, char c) {
        int a = ((header[0] & 0xFF) << 8) | (header[1] & 0xFF);
        int b = (header[0] << 8) | (header[1] & 0xff);
        byte d = (byte) (flags | mask);
        int e = x | header[0];
        String s = "len " + (x << 8) + header[1];
        int f = (x << 4) | header[0];
        int g = (c << 8) | c;
        return a + b + d + e + f + g + s.length();
    }

    static class Source extends InputStream {
        private final byte[] data = new byte[16];
        private int pos;

        @Override
        public int read() {
            if (pos >= data.length) {
                return -1;
            }
            return data[pos++];                                       // expect: byte-sign-extension
        }

        public int read(byte[] out) {
            return data[0];
        }

        int peek() {
            return data[pos];
        }
    }

    static class GoodSource extends InputStream {
        private final byte[] data = new byte[16];
        private int pos;

        @Override
        public int read() {
            Runnable r = () -> { byte x = data[0]; };
            return pos < data.length ? data[pos++] & 0xFF : -1;
        }
    }
}
