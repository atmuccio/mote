// mote-test: params java_release=17
// Release 17: these names would not compile, so the file is not Java 17 code as written;
// the rule only warns about releases the code has not reached yet.
class record {
}

class sealed {
}

class Modern {
    void m() {
        java.util.function.IntUnaryOperator f = _ -> 1;
    }
}
