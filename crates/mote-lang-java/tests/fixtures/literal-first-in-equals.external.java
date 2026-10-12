// mote-test: params external_constants=true
import com.acme.Names;
import com.acme.Dictionary;

class Lookups {
    static final String LOCAL = "l";

    boolean check(Dictionary dict, String name) {
        boolean a = dict.getItem(Names.TYPE).equals(Names.PAGE);   // expect: literal-first-in-equals
        boolean b = name.equals(Names.PAGE);                       // a variable receiver: not a lookup
        boolean c = dict.getItem(Names.TYPE).equals(Names.page);   // not an UPPER_SNAKE constant
        boolean d = dict.getName().trim().equals(Names.PAGE);      // trim() cannot return null
        boolean e = dict.getItem(Names.TYPE).equals(LOCAL);        // expect: literal-first-in-equals
        return a && b && c && d && e;
    }
}
