import java.nio.file.Path;

class Geometry {
    private final int width;
    private final int height;

    Geometry(int width, int height) {
        this.width = width;
        this.height = height;
    }

    Geometry(int size) {
        this(size, size);
    }

    static void copy(Path source, Path target) {}

    void resize(int width, int height) {}

    void positives(Path source, Path target, int width, int height, int depth) {
        copy(target, source);                                        // expect: swapped-arguments
        resize(height, width);                                       // expect: swapped-arguments
        this.resize(height, width);                                  // expect: swapped-arguments
        Geometry g = new Geometry(height, width);                    // expect: swapped-arguments
        between(1, depth, height, width);                            // expect: swapped-arguments
    }

    void between(int a, int depth, int width, int height) {}

    void negatives(Path source, Path target, int width, int height, int w, int h) {
        copy(source, target);
        resize(width, height);
        resize(h, w);
        resize(width, width);
        new Geometry(width, height);
        other.copy(target, source);
    }

    Geometry transpose() {
        return new Geometry(height, width);
    }

    void swapDimensions(int width, int height) {
        resize(height, width);
    }

    void resize(int width, int height, int depth) {
        resize(height, width);                                       // expect: swapped-arguments
    }

    Other other;

    static class Other {
        void copy(Path target, Path source) {}
    }
}

class Overloaded {
    void put(String key, String value) {}

    void put(Object value, Object key) {}

    void use(String key, String value) {
        put(value, key);
    }
}

class Inherits extends java.util.AbstractList<String> {
    void set(int index, int element) {}

    void call(int index, int element) {
        set(element, index);
    }

    public String get(int i) {
        return null;
    }

    public int size() {
        return 0;
    }
}

class Symmetric {
    int compare(String left, String right) {
        if (left == null) {
            return -compare(right, left);
        }
        return 0;
    }
}

class Relations {
    static boolean lt(int a, int b) {
        return a < b;
    }

    static boolean gt(int a, int b) {
        return lt(b, a);
    }

    static boolean contains(String outer, String inner) {
        return outer.contains(inner);
    }

    static int relate(String outer, String inner) {
        if (contains(outer, inner)) {
            return 1;
        }
        if (contains(inner, outer)) {
            return 2;
        }
        return 0;
    }

    static boolean matches(byte[] data, byte[] signature) {
        return data.length >= signature.length;
    }

    static boolean anyMatch(byte[][] signatures, byte[] data) {
        for (byte[] signature : signatures) {
            if (matches(signature, data)) {                         // expect: swapped-arguments
                return true;
            }
        }
        return false;
    }
}
