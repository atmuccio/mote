class NullInstanceof {
    private Object value;

    boolean positives(Object o, boolean flag, Holder h) {
        boolean a = o != null && o instanceof String;                   // expect: redundant-null-check-before-instanceof
        boolean b = null != o && o instanceof String s && s.isEmpty();  // expect: redundant-null-check-before-instanceof
        boolean c = flag && value != null && value instanceof Integer;  // expect: redundant-null-check-before-instanceof
        boolean d = (this.value != null) && (this.value instanceof Number);  // expect: redundant-null-check-before-instanceof
        boolean e = h.item != null && h.item instanceof String;         // expect: redundant-null-check-before-instanceof
        return a && b && c && d && e;
    }

    boolean negatives(Object o, Object p, Holder h) {
        boolean a = o instanceof String;
        boolean b = o != null && p instanceof String;
        boolean c = o != null || o instanceof String;
        boolean d = o == null || o instanceof String;
        boolean e = o instanceof String && o != null;
        boolean f = h.next() != null && h.next() instanceof String;     // calls may differ
        boolean g = o != null && flag() && o instanceof String;         // not adjacent
        return a && b && c && d && e && f && g;
    }

    boolean widened(Object o, Throwable t, Holder h, Object[] arr, int kind, boolean flag) {
        boolean a = o == null || !(o instanceof String);                // expect: redundant-null-check-before-instanceof
        boolean b = flag || null == o || !(o instanceof Number n);      // expect: redundant-null-check-before-instanceof
        boolean c = t.getCause() != null && t.getCause() instanceof RuntimeException; // expect: redundant-null-check-before-instanceof
        boolean d = h != null && h.getItem() != null && h.getItem() instanceof String; // expect: redundant-null-check-before-instanceof
        boolean e = o != null && kind == 3 && o instanceof String;      // expect: redundant-null-check-before-instanceof
        boolean f = o != null && t != null && (o instanceof Integer);   // expect: redundant-null-check-before-instanceof
        boolean g = arr[0] != null && arr[0] instanceof String;         // expect: redundant-null-check-before-instanceof
        boolean h1 = h.isReady() != null && h.isReady() instanceof Boolean; // expect: redundant-null-check-before-instanceof
        return a && b && c && d && e && f && g && h1;
    }

    boolean widenedNegatives(Object o, Holder h, Object[] arr, int i) {
        boolean a = o == null || o instanceof String;                   // not negated: a real alternative
        boolean b = o != null || !(o instanceof String);                // wrong operator for the form
        boolean c = o == null && !(o instanceof String);
        boolean d = o != null && o.hashCode() > 3 && o instanceof String; // uses o: relies on the check
        boolean e = o != null && h.touch() && o instanceof String;      // a call in between
        boolean f = h.fetch(1) != null && h.fetch(1) instanceof String; // call with arguments
        boolean g = h.next() != null && h.next() instanceof String;     // not a getter name
        boolean k = arr[i++] != null && arr[i++] instanceof String;     // side effects
        boolean m = o == null || !(p() instanceof String);
        return a && b && c && d && e && f && g && k && m;
    }

    Object p() { return null; }

    boolean flag() { return true; }

    static class Holder {
        Object item;
        Object next() { return item; }
        Object getItem() { return item; }
        Boolean isReady() { return true; }
        boolean touch() { item = null; return true; }
        Object fetch(int n) { return item; }
    }
}
