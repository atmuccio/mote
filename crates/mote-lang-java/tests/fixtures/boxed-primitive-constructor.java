import java.util.List;

class Boxing {
    Object positives(String text, int n, char c) {
        Integer a = new Integer(0);                     // expect: boxed-primitive-constructor
        Long b = new Long(n);                           // expect: boxed-primitive-constructor
        Short s = new Short((short) 1);                 // expect: boxed-primitive-constructor
        Byte y = new Byte((byte) 1);                    // expect: boxed-primitive-constructor
        Double d = new Double(1.5);                     // expect: boxed-primitive-constructor
        Float f = new Float(text);                      // expect: boxed-primitive-constructor
        Boolean flag = new Boolean(text);               // expect: boxed-primitive-constructor
        Character ch = new Character(c);                // expect: boxed-primitive-constructor
        Object o = new java.lang.Integer(3);            // expect: boxed-primitive-constructor
        return List.of(a, b, s, y, d, f, flag, ch, o);
    }

    Object negatives(String text) {
        Integer a = Integer.valueOf(0);
        Boolean flag = Boolean.valueOf(text);
        StringBuilder sb = new StringBuilder(text);
        Number n = new java.math.BigDecimal(text);
        return List.of(a, flag, sb, n);
    }

    static class Shadow {
        static class Integer {
            Integer(int v) {}
        }

        Object make() {
            return new Integer(5);                      // this file's own Integer class
        }
    }
}
