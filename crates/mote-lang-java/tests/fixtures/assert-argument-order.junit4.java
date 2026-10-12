// mote-test: test-file
import static org.junit.Assert.*;

import org.junit.Assert;
import org.junit.Test;

public class LegacyTest {
    @Test
    public void positives(Account account) {
        assertEquals(account.balance(), 100L);                      // expect: assert-argument-order
        assertEquals("balance", account.balance(), 100L);           // expect: assert-argument-order
        Assert.assertEquals(account.owner(), "ann");                // expect: assert-argument-order
        double rate = account.rate();
        assertEquals(rate, 0.5, 0.01);                              // expect: assert-argument-order
        assertEquals("rate", account.rate(), 0.5, 0.01);            // expect: assert-argument-order
    }

    @Test
    public void negatives(Account account, Object message) {
        assertEquals("balance", 100L, account.balance());           // message first, correct order
        assertEquals(100L, account.balance());
        assertEquals(message, account.balance(), 100L);             // (expected, actual, delta): correct order
    }
}
