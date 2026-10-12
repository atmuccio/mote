import java.util.List;

class DeclarationFarFromUse {
    void positives(List<String> items) {
        int retries = 0;                                      // expect: declaration-far-from-use
        String label;                                         // expect: declaration-far-from-use
        boolean found = false;                                // expect: declaration-far-from-use
        step();
        step();
        step();
        step();
        step();
        step();
        while (!ready() && retries < 3) {
            retries++;
        }
        label = "x";
        for (String item : items) {
            found |= item.isEmpty();
        }
    }

    void compliant(List<String> items) {
        long start = System.nanoTime();                       // a value that must not move
        int limit = items.size();                             // not a literal
        int close = 0;
        step();
        close++;
        int five = -1;
        step();
        step();
        step();
        step();
        step();
        five++;                                               // five statements between: accepted
        int unused = 0;                                       // never mentioned again: another rule's business
        int inBlock = 0;
        step();
        step();
        step();
        step();
        step();
        if (items.isEmpty()) {
            // a mention inside a statement counts as a use: inBlock
            step();
        }
        step();
        inBlock++;
        String lambda = null;
        step();
        step();
        step();
        step();
        step();
        Runnable r = () -> System.out.println(lambda);
        step();
        long elapsed = System.nanoTime() - start + limit;
    }

    DeclarationFarFromUse() {
        int x = 1;                                            // expect: declaration-far-from-use
        step();
        step();
        step();
        step();
        step();
        step();
        step(x);
    }

    void caseGroups(int k) {
        switch (k) {
            case 1:
                int y = 0;                                    // expect: declaration-far-from-use
                step();
                step();
                step();
                step();
                step();
                step();
                step(y);
                break;
            default:
                break;
        }
    }

    boolean ready() {
        return true;
    }

    void step() {
    }

    void step(int v) {
    }
}
