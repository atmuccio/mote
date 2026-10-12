import java.util.List;

class Chains {
    enum Kind { FILE, LINK, DIRECTORY, OTHER }

    String path;

    void positives(Kind kind, String name, int code) {
        if (kind == Kind.LINK) {
            path = follow(name);
            open(path);
        } else if (kind == Kind.FILE) {
            path = resolve(name);
            open(path);
        } else if (kind == Kind.DIRECTORY) {   // expect: duplicate-branch-body
            path = resolve(name);   // pasted from FILE
            open( path );
        }

        if (code == 1) {
            log("one");
            open(name);
        } else if (code == 2) {                 // expect: duplicate-branch-body
            log("one");
            open(name);
        } else {
            close();
        }

        if (code == 1) {
            log("one");
            open(name);
        } else if (code == 2) {                 // expect: duplicate-branch-body
            log("one");
            open(name);
        } else if (code == 3) {                 // expect: duplicate-branch-body
            log("one");
            open(name);
        } else if (code == 4) {
            close();
        }

        switch (code) {
            case 1:
                log("a");
                open(name);
                break;
            case 2:
                log("b");
                break;
            case 3:                             // expect: duplicate-branch-body
                log("a");
                open(name);
                break;
            default:
                close();
        }

        String r = switch (kind) {
            case FILE -> {
                log("f");
                yield resolve(name);
            }
            case LINK -> follow(name);
            case DIRECTORY -> {                 // expect: duplicate-branch-body
                log("f");
                yield resolve(name);
            }
            default -> name;
        };
    }

    void negatives(Kind kind, String name, int code, Object o, List<String> list) {
        // Short branches repeat on purpose.
        if (code == 1) {
            return;
        } else if (code == 2) {
            close();
        } else if (code == 3) {
            return;
        }
        // The last if/else pair is identical-branches' business.
        if (code == 1) {
            close();
        } else if (code == 2) {
            log("x");
            open(name);
        } else {
            log("x");
            open(name);
        }
        // Not neighbours: the branch in between may matter.
        if (kind == Kind.FILE) {
            path = resolve(name);
            open(path);
        } else if (kind == Kind.LINK) {
            path = follow(name);
            open(path);
        } else if (kind == Kind.DIRECTORY) {
            path = resolve(name);
            open(path);
        } else {
            path = follow(name);
            open(path);
        }
        // Different code.
        if (code == 1) {
            log("x");
            open(name);
        } else if (code == 2) {
            log("y");
            open(name);
        }
        // Pattern bindings: the same text may call different overloads.
        if (o instanceof Integer n) {
            log("n");
            use(n);
        } else if (o instanceof Long n) {
            log("n");
            use(n);
        }
        switch (o) {
            case Integer n -> {
                log("n");
                use(n);
            }
            case Long n -> {
                log("n");
                use(n);
            }
            default -> {}
        }
        // A group that falls through runs more than its own statements.
        switch (code) {
            case 1:
                log("a");
                open(name);
            case 2:
                log("b");
                break;
            case 3:
                log("a");
                open(name);
                break;
            default:
                break;
        }
        // Several labels on one group, one-line arms.
        switch (code) {
            case 1:
            case 2:
                log("a");
                open(name);
                break;
            case 3:
                return;
            case 4:
                return;
            default:
                break;
        }
        int v = switch (kind) {
            case FILE -> 1;
            case LINK -> 1;
            default -> 2;
        };
        // Identical empty branches are left to empty-block.
        if (code == 1) {
        } else if (code == 2) {
        }
    }

    void use(Integer i) {}
    void use(Long l) {}
    String resolve(String s) { return s; }
    String follow(String s) { return s; }
    void open(String s) {}
    void close() {}
    void log(String s) {}
}
