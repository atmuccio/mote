// mote-test: params max_statements=1
class DeclarationFarFromUseParams {
    void method() {
        int a = 0;                                            // expect: declaration-far-from-use
        step();
        step();
        a++;
        int b = 0;
        step();
        b++;
    }

    void step() {
    }
}
