import java.io.DataInputStream;
import java.nio.ByteBuffer;

class IntToLongBitsToDouble {
    double wrong(DataInputStream buffer, int bits, Integer boxed, short s) {
        double a = Double.longBitsToDouble(buffer.readInt());      // expect: int-to-long-bits-to-double
        double b = Double.longBitsToDouble(0x3FF00000);            // expect: int-to-long-bits-to-double
        double c = Double.longBitsToDouble(bits);                  // expect: int-to-long-bits-to-double
        double d = java.lang.Double.longBitsToDouble(boxed);       // expect: int-to-long-bits-to-double
        double e = Double.longBitsToDouble(s);                     // expect: int-to-long-bits-to-double
        return a + b + c + d + e;
    }

    // Compliant
    double fine(DataInputStream buffer, long bits, int i, Unknown u) {
        double a = Double.longBitsToDouble(buffer.readLong());
        double b = Double.longBitsToDouble(0x3FF0000000000000L);
        double unknown = Double.longBitsToDouble(ByteBuffer.allocate(8).getInt());
        double c = Double.longBitsToDouble(bits);
        double d = Double.longBitsToDouble(0);
        double e = Double.longBitsToDouble((long) i << 32);
        float f = Float.intBitsToFloat(i);
        double g = Double.longBitsToDouble(u.bits());
        double h = Double.longBitsToDouble(i & 0xFFFFFFFFL);
        return a + b + c + d + e + f + g + h + unknown;
    }

    interface Unknown { long bits(); }
}
