class Counter {
    private int count;
    private int[] slots = new int[3];

    void wrong(int retries) {
        retries = retries++;                 // expect: post-increment-self-assignment
        count = count--;                     // expect: post-increment-self-assignment
        this.count = this.count++;           // expect: post-increment-self-assignment
        retries = (retries++);               // expect: post-increment-self-assignment
    }

    void fine(int retries, int other) {
        retries++;
        retries = ++retries;                 // prefix stores the new value
        retries = other++;
        count = retries++;
        slots[0] = slots[0]++ + 0;
        int copy = retries++;
        retries = retries + 1;
    }
}
