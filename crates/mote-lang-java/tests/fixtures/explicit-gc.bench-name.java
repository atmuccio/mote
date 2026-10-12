// mote-test: path core/src/main/java/org/acme/bench/MemoryBench.java
package org.acme.bench;

// Named like a benchmark but production code without JMH: still reported.
public class MemoryBench {
    static void settle() {
        System.gc();                                               // expect: explicit-gc
    }
}
