import java.util.ArrayList;
import java.util.List;
import java.util.Map;

class Casts<T> {
    private String label;
    private T item;
    private Object anything;

    void positives(List<String> names, Map<String, Integer> counts, long count, int size, char c) {
        String first = (String) names.get(0);                   // expect: redundant-cast
        Integer n = (Integer) counts.get("a");                 // expect: redundant-cast
        long total = (long) count * size;                       // expect: redundant-cast
        String s = (String) ("a" + size);                        // expect: redundant-cast
        String l = (String) this.label;                          // expect: redundant-cast
        char d = (char) c;                                       // expect: redundant-cast
        List<String> same = (List<String>) names;                // expect: redundant-cast
        int rounded = (int) Math.round(1.5f);                    // expect: redundant-cast
        Object o = (Object) anything;                            // expect: redundant-cast
        String[] arr = new String[0];
        Object copy = (String[]) arr;                            // expect: redundant-cast
    }

    void negatives(List<String> names, List raw, Object value, int size, long big, double ratio) {
        String a = (String) value;                  // narrowing
        Object b = (Object) names;                  // widening
        long c = (long) size;                       // widening
        int d = (int) big;                          // narrowing
        long e = Math.round(ratio);
        int f = (int) Math.round(ratio);            // round(double) returns long
        String g = (String) null;                   // picks a type for null
        Runnable h = (Runnable) () -> { };          // gives the lambda its type
        T i = (T) item;                             // type variable: unknown
        String j = (String) raw.get(0);             // raw list returns Object
        List<String> k = (List<String>) (List) raw; // double cast idiom
        Integer m = (Integer) size;                 // boxing
        int p = (int) Integer.valueOf(3);           // unboxing
        byte q = (byte) 1;                          // literal is an int
        List<String> r = (List<String>) new ArrayList<String>();  // ArrayList to List
        Comparable<String> s = (Comparable<String> & java.io.Serializable) String::compareTo;
        Object t = (Object) service.call();         // unknown type
        Object t2 = (Object) unknownCall();         // expect: redundant-cast
        String u = (String) lambdaParam(x -> (String) x);
        int v = (int) Math.round(external.size() * 0.5);   // unknown operand: overload unsure
        int w = (int) Math.round(external.ratio());        // unknown argument
        long z = 5;
        java.util.function.IntUnaryOperator op = cur -> (int) Math.max(0, cur - z); // lambda parameter
        int y = (int) Math.min(size, external.length());   // min(long, long) if length is long
    }

    Object unknownCall() { return null; }

    Object lambdaParam(java.util.function.Function<Object, String> f) { return null; }
}
