// mote-test: path bench-it/src/main/java/org/acme/bench/CacheBench.java
package org.acme.bench;

import java.util.concurrent.TimeUnit;
import org.openjdk.jmh.annotations.*;

@State(Scope.Benchmark)
@OutputTimeUnit(TimeUnit.MILLISECONDS)
public class CacheBench {
    // Collecting before measuring memory is part of a benchmark's method.
    static long usedMemory() {
        System.gc();
        Runtime rt = Runtime.getRuntime();
        rt.gc();
        return rt.totalMemory() - rt.freeMemory();
    }

    @Benchmark
    public long fill() {
        return usedMemory();
    }
}
