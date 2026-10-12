import java.io.BufferedReader;
import java.io.IOException;

class ChainedAssignment {
    Object first;
    Object last;
    int width;
    int height;

    void reset() {
        first = last = null;                                  // expect: chained-assignment
        this.width = this.height = 0;                         // expect: chained-assignment
        int a, b;
        a = b = 1;                                            // expect: chained-assignment
        int c = width = 5;                                    // expect: chained-assignment
        width += height = 2;                                  // expect: chained-assignment
        a = (b = 3);                                          // expect: chained-assignment
    }

    void compliant(BufferedReader in) throws IOException {
        first = null;
        last = null;
        String line;
        while ((line = in.readLine()) != null) {
            width++;
        }
        int x = width + (height = 2);
        boolean changed = (width = 3) > 0;
        Runnable r = () -> width = 1;
        for (int i = 0, j = 0; i < 3; i = j = i + 1) {
            height++;
        }
    }
}
