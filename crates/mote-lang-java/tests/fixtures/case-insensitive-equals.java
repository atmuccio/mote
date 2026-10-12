import java.util.Locale;
import java.util.Objects;

class Headers {
    boolean positives(String header, String expected, String answer) {
        boolean a = header.toLowerCase().equals(expected.toLowerCase());          // expect: case-insensitive-equals
        boolean b = header.toUpperCase().equals(expected.toUpperCase());           // expect: case-insensitive-equals
        boolean c = answer.toLowerCase().equals("yes");                            // expect: case-insensitive-equals
        boolean d = "NO".equals(answer.toUpperCase());                             // expect: case-insensitive-equals
        boolean g = answer.toUpperCase(Locale.getDefault()).equals("JMS");         // expect: case-insensitive-equals
        int e = header.toLowerCase().compareTo(expected.toLowerCase());            // expect: case-insensitive-equals
        boolean f = (header.trim().toLowerCase()).equals("content-type");          // expect: case-insensitive-equals
        return a || b || c || d || e == 0 || f || g;
    }

    boolean negatives(String header, String expected, Object other, Token token) {
        boolean a = header.toLowerCase().equals(expected);           // the other side may be mixed case
        boolean b = header.toLowerCase().equals(expected.toUpperCase()); // different cases (another problem)
        boolean c = header.toLowerCase().equals("Yes");              // literal in mixed case
        boolean d = header.toLowerCase().equals("stra\u00dfe");      // escapes: left alone
        boolean e = header.toLowerCase().equals("é");                // non-ASCII letter
        boolean f = Objects.equals(header.toLowerCase(), expected.toLowerCase());
        boolean g = header.toLowerCase().startsWith(expected.toLowerCase());
        boolean h = header.equalsIgnoreCase(expected);
        boolean i = token.toLowerCase().equals(expected.toLowerCase()); // not a String
        boolean j = header.toLowerCase().equals("123");              // no letters
        int k = header.toLowerCase().compareTo("abc");                // compareTo with a literal
        boolean l = header.toLowerCase(Locale.ROOT).equals(expected.toLowerCase(Locale.ROOT)); // explicit locale
        int m = header.toUpperCase().compareTo(expected.toUpperCase()); // different order for _
        boolean n = header.toLowerCase(Locale.ROOT).equals("yes");  // explicit locale: whole-string mapping chosen
        boolean o = "NO".equals(header.toUpperCase(Locale.ENGLISH)); // explicit locale
        return a || b || c || d || e || f || g || h || i || j || k == 0 || l || m == 0 || n || o;
    }

    interface Token {
        Token toLowerCase();
    }
}
