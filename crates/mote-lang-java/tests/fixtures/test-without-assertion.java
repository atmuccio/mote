// mote-test: test-file
import static org.assertj.core.api.Assertions.assertThat;
import static org.hamcrest.MatcherAssert.assertThat;
import static org.junit.jupiter.api.Assertions.*;
import static org.mockito.Mockito.*;
import static org.mockito.BDDMockito.then;

import org.junit.jupiter.api.Disabled;
import org.junit.jupiter.api.Nested;
import org.junit.jupiter.api.Test;

class ParserTest {
    private final Parser parser = new Parser();

    @Test
    void parsesHeader() {                                           // expect: test-without-assertion
        parser.parse("id;name");
    }

    @Test
    public void empty() { }                                         // expect: test-without-assertion

    @Test
    void helperWithoutAssertion() {                                 // expect: test-without-assertion
        prepare();
        parser.parse("x");
    }

    @Test
    void qualifiedHelperUnknown() {                                 // expect: test-without-assertion
        fixture.run();
    }

    @Test
    void stubsOnly() {                                              // expect: test-without-assertion
        Service s = mock(Service.class);
        when(s.name()).thenReturn("x");
        parser.use(s);
    }

    // ---- negatives: every style of assertion
    @Test
    void junit5() {
        assertEquals(2, parser.parse("a;b").size());
    }

    @Test
    void assertJ() {
        assertThat(parser.parse("a")).isNotEmpty();
    }

    @Test
    void assertJQualified() {
        org.assertj.core.api.Assertions.assertThat(1).isOne();
    }

    @Test
    void hamcrest() {
        assertThat(parser.parse("a"), hasSize(1));
    }

    @Test
    void mockitoVerify() {
        Service s = mock(Service.class);
        parser.use(s);
        verify(s).name();
    }

    @Test
    void bdd() {
        Service s = mock(Service.class);
        parser.use(s);
        then(s).should().name();
    }

    @Test
    void assertThrowsLambda() {
        assertThrows(IllegalArgumentException.class, () -> parser.parse(null));
    }

    @Test
    void failCall() {
        if (parser.broken()) {
            fail("broken");
        }
    }

    @Test
    void javaAssert() {
        assert parser.parse("a") != null;
    }

    @Test
    void throwsExplicitly() {
        if (parser.parse("a") == null) {
            throw new AssertionError("null");
        }
    }

    @Test
    void viaHelper() {
        checkParses("a;b");
    }

    @Test
    void viaNestedHelper() {
        level1();
    }

    @Test
    void softAssertions() {
        SoftAssertions softly = new SoftAssertions();
        softly.check(() -> parser.parse("a"));
        softly.assertAll();
    }

    @Test
    void mockMvc() throws Exception {
        mvc.perform(get("/")).andExpect(status().isOk());
    }

    @Test
    void methodReference() {
        parser.values().forEach(Assertions::assertNotNull);
    }

    @Disabled("flaky")
    @Test
    void disabled() {
        parser.parse("x");
    }

    void notATest() {
        parser.parse("x");
    }

    @Nested
    class Inner {
        @Test
        void usesOuterHelper() {
            checkParses("x");
        }

        @Test
        void innerWithout() {                                       // expect: test-without-assertion
            parser.parse("x");
        }
    }

    private void checkParses(String s) {
        assertNotNull(parser.parse(s));
    }

    private void level1() { level2(); }
    private void level2() { assertTrue(true); }
    private void prepare() { parser.reset(); }
}
