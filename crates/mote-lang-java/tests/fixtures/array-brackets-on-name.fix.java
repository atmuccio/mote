class Fixes {
    int counts [];                                        // expect: array-brackets-on-name
    int /* kept */ totals[];                              // expect: array-brackets-on-name
    int[] grid [] [];                                     // expect: array-brackets-on-name
    int first[], second;                                  // expect: array-brackets-on-name

    String names() [] {                                   // expect: array-brackets-on-name
        return new String[0];
    }

    <T> T[] items(T seed)[] {                             // expect: array-brackets-on-name
        return null;
    }
}
