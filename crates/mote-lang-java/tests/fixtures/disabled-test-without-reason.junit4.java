import org.junit.*;

public class LegacyTest {
    @Ignore                                                         // expect: disabled-test-without-reason
    @Test
    public void slow() { }

    @org.junit.Ignore                                               // expect: disabled-test-without-reason
    @Test
    public void qualified() { }

    @Ignore("needs a database")
    @Test
    public void db() { }
}
