import lombok.Getter;

// Lombok generates `getName()` on the outer class: its use inside Inner cannot be seen.
class WithLombok {
    @Getter
    private String name;

    private class Inner {
        String label() {
            return getName();
        }
    }
}
