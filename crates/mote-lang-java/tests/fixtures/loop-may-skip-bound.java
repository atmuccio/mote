import java.util.Iterator;

class LoopMaySkipBound {
    void positives(int[] data, int n) {
        for (int i = 0; i != data.length; i += 2) {                   // expect: loop-may-skip-bound
            use(data[i]);
        }
        for (int i = n; i != 0; i -= 3) {                             // expect: loop-may-skip-bound
            use(i);
        }
        for (int i = 0; i != n; i++) {                                // expect: loop-may-skip-bound
            if (data[i] == 0) {
                i++;
            }
        }
        for (long k = 1; k != 10; k = k + 2) {                        // expect: loop-may-skip-bound
            use((int) k);
        }
    }

    void negatives(int[] data, int n, Iterator<String> it, Node head) {
        for (int i = 0; i != n; i++) {
            use(i);
        }
        for (int i = 0; i < data.length; i += 2) {
            use(i);
        }
        for (int i = 0; i != 10; i += 2) {
            use(i);
        }
        for (int i = 10; i != 0; i -= 5) {
            use(i);
        }
        for (Node x = head; x != null; x = x.next) {
            use(0);
        }
        for (int i = 0; i != n; i++) {
            Runnable r = () -> use(1);
        }
    }

    static class Node {
        Node next;
    }

    void use(int x) {}
}
