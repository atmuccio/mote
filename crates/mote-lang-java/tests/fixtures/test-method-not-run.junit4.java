import org.junit.Test;

public class LegacyTest {
    @Test
    public static void staticTest() { }                             // expect: test-method-not-run

    @Test
    private void privateTest() { }                                  // expect: test-method-not-run

    @Test
    public void fine() { }
}

class OtherFramework {
    @com.example.Test
    private void notJunit() { }

    @Test2
    private int alsoNot() { return 0; }
}
