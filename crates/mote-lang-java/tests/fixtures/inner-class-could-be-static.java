import java.util.ArrayList;
import java.util.List;

class Cache<V> {
    private final List<String> keys = new ArrayList<>();
    private int hits;

    private class Entry {                                            // expect: inner-class-could-be-static
        String key;
        long expires;

        boolean expired(long now) {
            return now > expires;
        }
    }

    class Counter {                                                  // expect: inner-class-could-be-static
        int count;

        void add(Entry other) {
            count += other.key.length();
        }
    }

    private class UsesField {
        int size() {
            return keys.size();
        }
    }

    private class UsesMethod {
        void touch() {
            record();
        }
    }

    private class UsesOuterThis {
        Cache<V> owner() {
            return Cache.this;
        }
    }

    private class UsesTypeParameter {
        V value;
    }

    private class CreatesInner {
        Object make() {
            return new UsesField();
        }
    }

    private class Shadows {
        int hits;                                                    // same name as an outer field: conservative
    }

    private class InLambda {
        Runnable r = () -> hits++;
    }

    private static class AlreadyStatic {
        int x;
    }

    public class PublicApi {
        int x;
    }

    void record() {
        hits++;
    }

    interface Callback {
        class ImplicitlyStatic {
            int x;
        }
    }

    void local() {
        class Local {
            int x;
        }
        Object anon = new Object() {
            class InAnonymous {
                int y;
            }
        };
    }
}

class Derived extends Cache<String> {
    private class MayUseInherited {
        int n() {
            return 1;
        }
    }
}

enum Level {
    LOW;

    private int weight;

    private class Holder {                                           // expect: inner-class-could-be-static
        String label;
    }

    private class UsesWeight {
        int w() {
            return weight;
        }
    }
}

class Outer {
    private int state;

    private static class StaticMiddle {
        private class Leaf {                                         // expect: inner-class-could-be-static
            int v;
        }
    }

    private class InnerMiddle {
        private class Leaf2 {
            int v() {
                return state;
            }
        }
    }
}

class JUnitStyle {
    @org.junit.jupiter.api.Nested
    class WhenEmpty {
        void test() {}
    }
}
