sealed interface Shape permits Circle, Square {}
record Circle(double r) implements Shape {}
record Square(double s) implements Shape {}

class Modern {
    String describe(Object o, String expected) {
        String text = """
            multi-line == text
            """;
        if (o instanceof String s && s == expected) {             // expect: string-reference-equality
            return s;
        }
        return switch (o) {
            case Circle c when c.r() > 1 -> "big";
            case String str -> str == text ? "same" : "diff";     // expect: string-reference-equality
            default -> {
                var label = text.strip();
                yield label == expected ? "x" : "y";              // expect: string-reference-equality
            }
        };
    }

    boolean records(Circle a, Circle b) {
        return a == b;
    }
}
