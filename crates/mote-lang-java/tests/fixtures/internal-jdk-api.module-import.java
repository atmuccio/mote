import module java.base;
import module jdk.internal.vm.ci;
import sun.misc.Unsafe;                                            // expect: internal-jdk-api

// A module import (Java 25) names a module, not a package or a class: it is not reported,
// even when the module name looks like an internal package.
class Modules {
    Unsafe unsafe;
}
