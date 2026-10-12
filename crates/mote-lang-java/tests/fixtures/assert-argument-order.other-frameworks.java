// mote-test: test-file
import static org.testng.Assert.assertEquals;

import org.testng.annotations.Test;

class TestNgOrderTest {
    @Test
    void testng(Account account) {
        assertEquals(account.balance(), 100L);                      // TestNG: (actual, expected) is right
        org.testng.Assert.assertEquals(account.owner(), "ann");
    }
}

class OwnHelperTest {
    void assertEquals(Object a, Object b) { }

    void m(Account account) {
        assertEquals(account.balance(), 100L);                      // own helper
    }
}

class ProductionLike {
    void m(Account account, Checker checker) {
        checker.assertEquals(account.balance(), 100L);              // unknown owner
    }
}
