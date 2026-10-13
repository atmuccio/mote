// Java 25: a compact source file (JEP 512) with an instance `main`, a field, a helper method,
// a nested record, and Markdown documentation comments (Java 23, JEP 467).
import module java.base;

/// The greeting printed by [#main()].
final String greeting = "hello";

/// Prints the greeting and the number of names.
///
/// - one
/// - two
void main() {
    IO.println(greeting + " " + count(List.of("a", "b")));
}

int count(List<String> names) {
    return names.size();
}

record Pair(String left, String right) {}
