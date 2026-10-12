import java.util.Arrays;
import java.util.List;

record Samples(String name, double[] values) { }                     // expect: record-array-component

record Parts(int id, String... parts) { }                           // expect: record-array-component

record Grid(int[][] cells) {                                        // expect: record-array-component
    @Override
    public boolean equals(Object o) {
        return o instanceof Grid g && Arrays.deepEquals(cells, g.cells);
    }
}

class Outer {
    record Nested(byte[] data) { }                                  // expect: record-array-component
}

// Compliant --------------------------------------------------------------------
record Safe(List<Double> values) { }

record Plain(int a, String b) { }

record Handled(char[] chars) {
    @Override
    public boolean equals(Object o) {
        return o instanceof Handled h && Arrays.equals(chars, h.chars);
    }

    @Override
    public int hashCode() {
        return Arrays.hashCode(chars);
    }
}

class NotARecord {
    int[] values;
}
