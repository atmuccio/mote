// mote-test: test-file
import static org.junit.jupiter.api.Assertions.assertDoesNotThrow;
import static org.junit.jupiter.api.Assertions.assertThrows;

import java.util.Arrays;
import java.util.List;
import org.junit.jupiter.api.Assertions;
import org.junit.jupiter.api.Test;

class ParserTest {
    @Test
    void positives(Config config, Parser parser) {
        assertThrows(ParseException.class, () -> {                  // expect: assert-throws-multiple-calls
            Parser p = new Parser(config);
            p.parse("{");
        });
        assertThrows(ParseException.class, () -> parser.parse(readFixture("broken.json"))); // expect: assert-throws-multiple-calls
        Assertions.assertThrows(IllegalStateException.class, () -> parser.apply(new Rule(config))); // expect: assert-throws-multiple-calls
        assertDoesNotThrow(() -> {                                  // expect: assert-throws-multiple-calls
            parser.reset();
            parser.parse("{}");
        });
    }

    @Test
    void negatives(Parser parser, String input) {
        assertThrows(ParseException.class, () -> parser.parse("{"));
        assertThrows(ParseException.class, () -> parser.parse(input));
        assertThrows(ParseException.class, () -> { parser.parse(input); });
        assertThrows(ParseException.class, () -> parser.parse(input.trim()));      // getter-like call
        assertThrows(ParseException.class, () -> parser.parseAll(List.of("a", "b"))); // JDK factory
        assertThrows(ParseException.class, () -> parser.parseAll(Arrays.asList(input)));
        assertThrows(ParseException.class, () -> parser.read(new java.io.StringReader(input)));
        assertThrows(ParseException.class, () -> parser.handle(x -> x.parse(input))); // nested lambda
        assertThrows(ParseException.class, parser::fail);
        assertDoesNotThrow(() -> parser.parse("{}"));
        assertThrows(ParseException.class, () -> {
            parser.parse(input);
            return null;                                            // only satisfies ThrowingSupplier
        });
        assertThrows(ParseException.class, () -> parser.configure(new Settings(), input)); // no-arg constructor
        assertDoesNotThrow(() -> Assertions.assertNotNull(new Parser(input))); // the assertion is not setup
    }

    @Test
    void assertionWithSetup(Parser parser, String input) {
        assertDoesNotThrow(() -> Assertions.assertNotNull(new Parser(load(input)))); // expect: assert-throws-multiple-calls
        assertThrows(ParseException.class, () -> {                  // expect: assert-throws-multiple-calls
            parser.reset();
            parser.parse(input);
            return null;
        });
    }
}
