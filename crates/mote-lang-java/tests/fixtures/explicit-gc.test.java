// mote-test: test-file
class MemoryTest {
    void weakReferencesAreCleared() {
        System.gc();
        Runtime.getRuntime().gc();
    }
}
