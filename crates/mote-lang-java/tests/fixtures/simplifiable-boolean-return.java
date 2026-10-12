import java.util.List;

class Bools {
    boolean a(List<String> list) {
        if (list.isEmpty()) {                                // expect: simplifiable-boolean-return
            return true;
        } else {
            return false;
        }
    }

    boolean b(int x) {
        if (x > 0) return false;                             // expect: simplifiable-boolean-return
        else return true;
    }

    boolean c(int x) {
        if (x > 0) {                                         // expect: simplifiable-boolean-return
            return true;
        }
        return false;
    }

    boolean d(int x) {
        if (x > 0) {                                         // expect: simplifiable-boolean-return
            return false;
        }
        // comment between
        return true;
    }

    boolean e(int x) {
        if (x > 0) {
            return true;
        }
        return true;
    }

    boolean f(int x) {
        if (x > 0) {
            log();
            return true;
        }
        return false;
    }

    Boolean g(int x) {
        if (x > 0) {
            return true;
        }
        log();
        return false;
    }

    boolean h(int x) {
        if (x > 0) {
            return true;
        } else if (x < -5) {                                 // expect: simplifiable-boolean-return
            return true;
        } else {
            return false;
        }
    }

    boolean i(int x) {
        while (x > 0)
            if (x-- > 10) return true;
        return false;
    }

    int a, b;

    @Override
    public boolean equals(Object o) {
        if (!(o instanceof Bools)) {
            return false;
        }
        Bools other = (Bools) o;
        if (a != other.a) {
            return false;
        }
        if (b != other.b) {
            return false;
        }
        return true;
    }

    boolean afterOtherStatement(int x) {
        log();
        if (x > 3) return false;                             // expect: simplifiable-boolean-return
        return true;
    }

    void log() {}
}

class Guards {
    boolean sameValueChain(String s) {
        if (s.startsWith("a")) {
            return true;
        }
        if (s.startsWith("b")) {
            return true;
        }
        return false;
    }

    boolean oppositeGuard(String s) {
        if (s == null) {
            return false;
        }
        if (s.isEmpty()) {                                   // expect: simplifiable-boolean-return
            return true;
        }
        return false;
    }

    boolean oppositeGuardNegated(Object o) {
        if (o == null) return true;
        if (o instanceof String) return false;               // expect: simplifiable-boolean-return
        return true;
    }

    boolean parenthesized(int x) {
        if (x < 0) {
            return (false);
        }
        if (x > 10) {                                        // expect: simplifiable-boolean-return
            return (true);
        }
        return (false);
    }

    boolean guardWithElse(int x) {
        if (x < 0) {
            return false;
        } else {
            log(x);
        }
        if (x > 10) {                                        // expect: simplifiable-boolean-return
            return true;
        }
        return false;
    }

    boolean notALiteral(int x) {
        if (x < 0) {
            return false;
        }
        if (x > 10) {
            return true;
        }
        return x == 5;
    }

    void log(int x) {}
}

class GuardChains {
    int id;
    String name;
    int[] parts;

    @Override
    public boolean equals(Object obj) {
        if (this == obj) {
            return true;
        }
        if (obj == null || getClass() != obj.getClass()) {
            return false;
        }
        final GuardChains other = (GuardChains) obj;
        if (id != other.id) {
            return false;
        }
        return true;
    }

    boolean sameName(Object obj) {
        if (!(obj instanceof GuardChains)) return false;
        GuardChains other = (GuardChains) obj;
        // a comment
        int n = 3;
        if (name == null) {
            if (other.name != null) {
                return false;
            }
        } else if (!name.equals(other.name)) {
            return false;
        }
        if (!java.util.Arrays.equals(parts, other.parts)) {
            return false;
        }
        return true;
    }

    boolean branchDoesWork(Object obj) {
        if (name == null) {
            log();
        } else if (name.isEmpty()) {
            return false;
        }
        if (id > 3) {                                        // expect: simplifiable-boolean-return
            return false;
        }
        return true;
    }

    boolean declarationAfterOpposite(Object obj) {
        if (obj == null) {
            return true;
        }
        GuardChains other = (GuardChains) obj;
        if (other.id == id) {                                // expect: simplifiable-boolean-return
            return false;
        }
        return true;
    }

    boolean elseBranchReturnsOpposite(int x) {
        if (x < 0) {
            return false;
        } else if (x > 100) {
            return true;
        }
        if (x == 5) {                                        // expect: simplifiable-boolean-return
            return false;
        }
        return true;
    }

    void log() {}
}
