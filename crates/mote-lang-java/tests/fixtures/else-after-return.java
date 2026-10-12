class Elses {
    String a(String value) {
        if (value == null) {
            return "default";
        } else {                                             // expect: else-after-return
            return value.trim();
        }
    }

    void b(Object o) {
        if (o == null) throw new IllegalArgumentException();
        else o.hashCode();                                   // expect: else-after-return
    }

    int c(int x) {
        if (x > 0) {
            return 1;
        } else if (x < 0) {                                  // else-if chains are not reported
            return -1;
        } else {
            return 0;
        }
    }

    int d(int x) {
        if (x > 0) {
            x++;
        } else {
            return 0;
        }
        return x;
    }

    int e(int x) {
        if (x > 0) {
            if (x > 10) {
                return 10;
            }
        } else {
            return 0;
        }
        return x;
    }

    int f(int x) {
        if (x > 0) {
            return 1;
            // trailing comment after the return
        } else {                                             // expect: else-after-return
            x--;
        }
        return x;
    }

    void g(boolean flag) {
        for (int i = 0; i < 3; i++) {
            if (flag) {
                break;
            } else {
                continue;
            }
        }
    }
}
