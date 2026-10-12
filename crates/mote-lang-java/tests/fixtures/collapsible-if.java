class Ifs {
    void m(java.io.File file, boolean a, boolean b) {
        // expect-next-line: collapsible-if
        if (file != null) {
            if (file.exists()) {
                load(file);
            }
        }
        if (a)                                               // expect: collapsible-if
            if (b) load(null);
        if (a) {
            if (b) {
                load(null);
            } else {
                load(file);
            }
        }
        if (a) {
            if (b) load(null);
        } else {
            load(file);
        }
        if (a) {
            // the outer condition deserves its own explanation
            if (b) load(null);
        }
        if (a) {
            if (b) load(null);
            load(file);
        }
        if (a) if (b) if (file.exists()) load(file);         // expect: collapsible-if, collapsible-if
        if (a) {
            load(file);
        // expect-next-line: collapsible-if
        } else if (b) {
            if (file.canRead()) load(file);
        }
    }

    void load(java.io.File f) {}
}
