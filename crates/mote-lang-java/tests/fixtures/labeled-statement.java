class Labels {
    Cell find(Row[] rows, String query) {
        Cell found = null;
        search:                                                     // expect: labeled-statement
        for (Row row : rows) {
            for (Cell cell : row.cells()) {
                if (cell.matches(query)) {
                    found = cell;
                    break search;
                }
            }
        }
        outer: while (true) {                                       // expect: labeled-statement
            inner: do {                                             // expect: labeled-statement
                continue outer;
            } while (false);
        }
    }

    void negatives(int x) {
        switch (x) {
            case 1:
                break;
            default:
                break;
        }
        String s = x > 0 ? "a" : "b";
        Runnable r = () -> { };
        // label: not a statement
        String text = "label: for (;;) {}";
    }
}
