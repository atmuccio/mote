import javax.annotation.Nonnull;
import javax.annotation.Nullable;
import org.jetbrains.annotations.NotNull;

class Account {
    @Nullable int retries;                                         // expect: nullability-annotation-misuse
    @NotNull private boolean active;                               // expect: nullability-annotation-misuse
    private @Nullable long a, b;                                   // expect: nullability-annotation-misuse

    // Boxed types and arrays can be null: fine.
    @Nullable Integer boxed;
    @Nullable int[] codes;
    @Nullable int legacy[];
    @Nullable String name;

    @Nullable                                                      // expect: nullability-annotation-misuse
    int count() { return 0; }

    @Nonnull double ratio() { return 1.0; }                        // expect: nullability-annotation-misuse

    @Nullable void reset() { }                                     // expect: nullability-annotation-misuse

    @Nullable int[] table() { return null; }

    void update(@Nullable int delta, @NotNull String reason) { }   // expect: nullability-annotation-misuse

    void more(final @org.checkerframework.checker.nullness.qual.Nullable char c) { } // expect: nullability-annotation-misuse

    void local() {
        @Nullable int x = 0;                                       // expect: nullability-annotation-misuse
        @Nullable Object o = null;
        java.util.function.IntUnaryOperator f = (@Nullable int v) -> v; // expect: nullability-annotation-misuse
    }

    // Other annotations on primitives are fine.
    @Deprecated int old;
    @SuppressWarnings("unused") int unused;

    @Override
    public boolean equals(@Nonnull Object other) {                  // expect: nullability-annotation-misuse
        return this == other;
    }
}

record Point(@Nullable int x, int y) { }                           // expect: nullability-annotation-misuse

class Fine {
    @Override
    public boolean equals(@Nullable Object other) { return false; }

    public boolean equals(@NonNull Fine other) { return false; }    // overload, not the Object contract
}

class Other {
    boolean equals(@NonNull Object a, Object b) { return false; }  // not equals(Object)
}

interface Repo {
    @Nullable String find(@Nullable String key);
    @NonNull String require(String key);
}

class NarrowRepo implements Repo {
    @Override
    public @Nullable String find(@NonNull String key) { return null; } // expect: nullability-annotation-misuse

    @Override
    public @Nullable String require(String key) { return null; }   // expect: nullability-annotation-misuse
}

class WideRepo implements Repo {
    @Override
    public @NonNull String find(@Nullable String key) { return ""; }

    @Override
    public String require(@Nullable String key) { return ""; }
}

class Base {
    void put(@Nullable String k) { }
}

class Derived extends Base {
    @Override
    void put(@NonNull String k) { }                                // expect: nullability-annotation-misuse

    void put(@NonNull Object k) { }                                // overload: fine
}

class UnknownParent extends java.util.AbstractList<String> {
    @Override
    public @Nullable String get(@NonNull int index) { return null; } // expect: nullability-annotation-misuse

    @Override
    public int size() { return 0; }
}
