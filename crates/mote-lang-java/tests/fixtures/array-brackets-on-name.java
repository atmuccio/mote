class ArrayBracketsOnName {
    private int counts[];                                     // expect: array-brackets-on-name
    private int[] good;
    private int[] grid[];                                     // expect: array-brackets-on-name
    private int a, b[];                                       // expect: array-brackets-on-name

    public static void main(String args[]) {                  // expect: array-brackets-on-name
        String parts[] = args;                                // expect: array-brackets-on-name
        String[] fine = args;
        int[] created = new int[3];
        int[][] matrix = new int[2][2];
        int value = created[0];
        Runnable r = () -> {
            long longs[] = { 1L };                            // expect: array-brackets-on-name
        };
    }

    int values()[] {                                          // expect: array-brackets-on-name
        return new int[0];
    }

    int[] goodValues() {
        return new int[0];
    }

    void varargs(String... names) {
    }

    record Pair(int left[], int[] right) {                    // expect: array-brackets-on-name
    }

    @interface Ann {
        int[] ok();
        String bad()[];                                       // expect: array-brackets-on-name
    }
}
