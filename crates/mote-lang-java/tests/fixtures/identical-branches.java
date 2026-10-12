class Branches {
    String positives(boolean admin, boolean compact, String name, String page) {
        if (admin) {                                    // expect: identical-branches
            render(page);
        } else {
            render(page);
        }
        if (compact) render(page); else render(page);   // expect: identical-branches
        if (admin) {                                    // expect: identical-branches
            render(page); // shown to admins
        } else {
            render( page ); /* shown to others */
        }
        if (admin) {
            render("a");
        } else if (compact) {                           // expect: identical-branches
            render(name);
        } else {
            render(name);
        }
        String label = compact ? name : name;           // expect: identical-branches
        return admin ? (name) : name;                   // expect: identical-branches
    }

    String negatives(boolean admin, boolean compact, String name, String page) {
        if (admin) {
            render(page);
        } else {
            render(name);
        }
        if (admin) {
            render(page);
        } else if (compact) {
            render(page);
        }
        if (admin) {
            render(page);
        }
        if (admin) {
            // nothing yet
        } else {
            // nothing yet
        }
        if (admin) {
        } else {
        }
        String s = admin ? "a b" : "ab";
        Runnable r = admin ? () -> render(page) : () -> render(name);
        return compact ? name.trim() : name;
    }

    void render(String s) {}
}
