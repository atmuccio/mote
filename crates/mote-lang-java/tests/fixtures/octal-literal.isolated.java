// Two literals below the file convention: reported.
class Paging {
    int limit = 0100;                                               // expect: octal-literal

    int month() {
        return 012;                                                 // expect: octal-literal
    }
}
