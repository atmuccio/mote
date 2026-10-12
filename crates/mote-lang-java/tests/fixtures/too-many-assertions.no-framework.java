// mote-test: path src/main/java/com/acme/Checks.java
// mote-test: params max=2
class Checks {
    // A production class with its own `Test` annotation and assertion helpers.
    @interface Test {
    }

    @Test
    void verify() {
        assertValid(1);
        assertValid(2);
        assertValid(3);
    }

    static void assertValid(int v) {
    }
}
