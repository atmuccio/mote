class Totals {
    int total;
    boolean done;

    void wrong(int price, boolean ok) {
        total =+ price;                      // expect: reversed-compound-assignment
        total =- 1;                          // expect: reversed-compound-assignment
        done =! ok;                          // expect: reversed-compound-assignment
        this.total =+ 2;                     // expect: reversed-compound-assignment
    }

    void fine(int price, boolean ok) {
        total += price;
        total -= 1;
        total = -1;
        total = +price;
        total=-1;
        total =-1;
        done = !ok;
        done = ! ok;
        int offset = -1;
        total = - price;
    }
}
