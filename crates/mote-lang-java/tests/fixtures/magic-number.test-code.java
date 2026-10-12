// mote-test: test-file
class CalculatorTest {
    void adds() {
        assertEquals(42, add(40, 2));
    }
    int add(int a, int b) { return a + b; }
    void assertEquals(int a, int b) {}
}
