// mote-test: params report_array_index=true
class Buffer {
    private int pos;

    void write(byte[] buf, byte b) {
        buf[pos++] = b;                                             // expect: assignment-in-operand
        byte c = buf[--pos];                                        // expect: assignment-in-operand
        pos++;
    }
}
