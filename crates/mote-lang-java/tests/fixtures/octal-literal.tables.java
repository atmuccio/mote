// mote-test: params report_tables=true file_convention=0
class Encoding {
    static final int[] CODES = {0101, 0102, 0103};                 // expect: octal-literal, octal-literal, octal-literal

    static final Object[][] NAMES = {
        {0101, "A"},                                                // expect: octal-literal
        {0102, "B"},                                                // expect: octal-literal
        {0103, "C"},                                                // expect: octal-literal
    };

    int small = 04;
}
