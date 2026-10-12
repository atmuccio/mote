// mote-test: params file_convention=0
import java.util.List;

class Schedule {
    static final int RETRY_SECONDS = 030;                           // expect: octal-literal
    static final long BIG = 0777L;                                  // a mask of sevens: octal on purpose

    int[] minutes() {
        return new int[] {5, 010, 15};                              // expect: octal-literal
    }

    void configure(Channel channel, List<Integer> ports) {
        channel.setMode(0644);                                      // a file mode: octal is the convention
        channel.setTimeout(0644);                                   // permission-shaped: a file mode anyway
        channel.setTimeout(0100);                                   // expect: octal-literal
        int mask = flags() & 0170000;                               // a bit mask
        int shifted = 1 << (flags() & 017);                         // a bit pattern
        int negative = -012;                                        // expect: octal-literal
        record(2024, 012, 01);                                      // expect: octal-literal
        int zip = 02134;                                            // expect: octal-literal
        int magic = 070707;                                         // five digits: nobody pads a decimal so
        int grouped = 0777_776;                                     // grouped digits: deliberate

        // Not reported.
        int zero = 0;
        int single = 07;                                            // same value in decimal
        int padded = 00;
        int hex = 0x1A4;
        int bin = 0b1010;
        long zeroL = 0L;
        double real = 012.5;                                        // a decimal floating literal
        double tiny = 0.5e-3;
        int decimal = 100;
        String text = "0644";
        char c = '\012';
    }

    // A lookup table written in octal on purpose.
    static final int[] CODES = {0101, 0102, 0103, 0104};

    static final Object[][] NAMES = {
        {0101, "A"},
        {0102, "B"},
        {0103, "C"},
        {0104, "D"},
    };

    int flags() {
        return 1;
    }

    void record(int y, int m, int d) {
    }

    interface Channel {
        void setMode(int mode);

        void setTimeout(int seconds);
    }
}
