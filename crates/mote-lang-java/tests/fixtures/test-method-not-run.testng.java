import org.testng.annotations.Test;

public class NgTest {
    @Test
    private void notJunit() { }                                     // TestNG is out of scope

    @Test
    public int returns() { return 1; }
}
