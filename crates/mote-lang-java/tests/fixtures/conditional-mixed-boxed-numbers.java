import java.io.Serializable;
import java.util.Map;

class ConditionalMixedBoxedNumbers {
    Integer limit;
    Long total;

    Number pick(boolean useLimit) {
        Number n = useLimit ? limit : total;                        // expect: conditional-mixed-boxed-numbers
        Object o = useLimit ? limit : 0L;                           // expect: conditional-mixed-boxed-numbers
        Long l = useLimit ? limit : total;                          // expect: conditional-mixed-boxed-numbers
        Serializable s;
        s = useLimit ? Double.valueOf(1.5) : limit;                 // expect: conditional-mixed-boxed-numbers
        consume(useLimit ? limit : total);                          // expect: conditional-mixed-boxed-numbers
        return useLimit ? limit : 2.5;                              // expect: conditional-mixed-boxed-numbers
    }

    long negatives(boolean f, Map<String, Integer> counts, int a, long b, Integer x, Integer y) {
        long sum = f ? limit : total;           // primitive target: conversion expected
        Integer same = f ? x : y;               // same wrapper type
        Integer withPrim = f ? x : 0;           // same numeric type
        Object prims = f ? a : b;               // both primitive
        long widened = f ? a : total;           // primitive target
        String text = f ? "a" : "b";
        Object mixed = f ? "a" : limit;         // not numeric
        Number fromMap = f ? counts.get("k") : x;
        var inferred = f ? limit : total;
        Object unknown = f ? helper() : limit;  // helper() resolves to Object
        return sum + widened;
    }

    Object helper() {
        return null;
    }

    void consume(Object o) {}
}
