// mote-test: test-file
// A shared base class named as one is not reported, declared abstract or not: its tests run
// in the subclasses, under their names.
import org.junit.jupiter.api.Test;

class LedgerTestBase {
    @Test
    void balancesStayEven() {
    }
}

class LedgerTestSupport {
    @Test
    void openingBalanceIsZero() {
    }
}

class AbstractLedgerChecks {
    @Test
    void postingsKeepTheirOrder() {
    }
}

class LedgerTest extends LedgerTestBase {
    @Test
    void closingBalanceMatches() {
    }
}

// `Abstract` has to be the whole first word, `TestBase` and `TestSupport` the whole ending.
class AbstractionChecks {                                     // expect: test-class-naming
    @Test
    void layersStaySeparate() {
    }
}

class LedgerBase {                                            // expect: test-class-naming
    @Test
    void balancesStayEven() {
    }
}

class LedgerSupport {                                         // expect: test-class-naming
    @Test
    void openingBalanceIsZero() {
    }
}

class LedgerTestBases {                                       // expect: test-class-naming
    @Test
    void balancesStayEven() {
    }
}

class LedgerTestSupporter {                                   // expect: test-class-naming
    @Test
    void openingBalanceIsZero() {
    }
}
