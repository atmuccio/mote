import java.util.Objects;

class NullEquals {
    private String name;

    boolean positives(Object result, NullEquals other, Unknown u) {
        boolean a = result.equals(null);                    // expect: equals-with-null
        boolean b = name.equals((null));                    // expect: equals-with-null
        boolean c = this.name.equals(null);                 // expect: equals-with-null
        boolean d = other.name.equals(null);                // expect: equals-with-null
        boolean e = u.value().equals(null);                 // expect: equals-with-null
        return a || b || c || d || e;
    }

    boolean negatives(Object result, String s) {
        boolean a = result == null;
        boolean b = Objects.equals(result, null);
        boolean c = s.equals("null");
        boolean d = s.equalsIgnoreCase(null);
        boolean e = Objects.isNull(result);
        return a || b || c || d || e || equals(null) || super.equals(null);
    }
}
