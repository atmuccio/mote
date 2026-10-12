// Three or more octal literals: the file writes octal on purpose, nothing is reported.
class CpioLike {
    static final int C_IRUSR = 0000400;
    static final int C_IWUSR = 0000200;
    static final int OLD_LIMIT = 0100;

    int check(int count) {
        return count > 010 ? 1 : 0;
    }
}

class Sibling {
    int limit = 030;
}
