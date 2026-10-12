import java.util.HashMap;
import java.util.Map;

class Charset {
    private static final Map<Integer, String> DIACRITICS = createDiacritics();
    static final byte[] CHARS = new byte[1 << 16];

    static {
        CHARS[3990] = 33;
        java.util.Arrays.fill(CHARS, 3974, 3980, (byte) -87);
    }

    private static Map<Integer, String> createDiacritics() {
        Map<Integer, String> map = new HashMap<>();
        map.put(0x02C8, "̍");
        map.put(0x0022, "̎");
        return map;
    }

    private static Map<Integer, String> calledTwice() {
        Map<Integer, String> map = new HashMap<>();
        map.put(0x00B4, "x");                                // expect: magic-number
        return map;
    }

    private final Map<Integer, String> other = calledTwice();

    void refresh() {
        calledTwice();
    }

    Charset() {
        Object[][] table = {
            {0, ".notdef"},
            {161, "degree"},
            {162, "thorn"},
        };
        int[] sizes = {16, 32, 64};
        int[] pair = {5, 6};                                 // expect: magic-number, magic-number
        int single = 37;                                     // expect: magic-number
    }
}
