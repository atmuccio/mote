public class Buffer {
    private int size;

    public void resize(int capacity) {
        assert capacity > 0 : "capacity must be positive";      // expect: assert-public-argument
        size = capacity;
    }

    public Buffer(String name, int limit) {
        assert name != null;                                     // expect: assert-public-argument
        assert limit >= 0 && limit < 100;                        // expect: assert-public-argument
        size = limit;
    }

    // Negatives.
    void internal(int capacity) {
        assert capacity > 0;
    }

    private void helper(int capacity) {
        assert capacity > 0;
    }

    public void later(int capacity) {
        size = 1;
        assert capacity > 0;                                     // not a precondition position
    }

    public void invariant(int capacity) {
        assert size >= 0;                                        // state, not the argument
    }

    public void mixed(int capacity) {
        assert capacity > size;                                  // compares with state
    }

    public void call(String s) {
        assert s.length() > 0;                                   // calls a method
    }
}

class Internal {
    public void resize(int capacity) {
        assert capacity > 0;                                     // class is not public
    }
}
