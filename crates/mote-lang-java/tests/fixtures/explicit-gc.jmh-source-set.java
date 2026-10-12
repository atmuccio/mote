// mote-test: path core/src/jmh/java/org/acme/bench/Harness.java
package org.acme.bench;

public class Harness {
    static void settle() {
        System.gc();
    }
}
