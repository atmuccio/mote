class Form {
    private boolean limited;
    private Button export;
    private Boolean boxed;

    void refresh(java.util.List<String> rows, int limit, Panel panel) {
        // expect-next-line: simplifiable-boolean-if-else
        if (rows.isEmpty()) {
            export.setEnabled(false);
        } else {
            export.setEnabled(true);
        }
        // expect-next-line: simplifiable-boolean-if-else
        if (limit > 0) {
            limited = true;
        } else {
            limited = false;
        }
        if (limit > 10) this.limited = true; else this.limited = false; // expect: simplifiable-boolean-if-else
        // expect-next-line: simplifiable-boolean-if-else
        if (panel.isOpen()) {
            panel.field.setVisible("name", true, 3);
        } else {
            panel.field.setVisible("name", false, 3);
        }

        // Not reported.
        if (rows.isEmpty()) {
            export.setEnabled(false);
        } else {
            export.setEnabled(true);
            export.repaint();                                       // more than one statement
        }
        if (rows.isEmpty()) {
            export.setEnabled(false);
        } else {
            // enabled once rows arrive
            export.setEnabled(true);
        }
        if (rows.isEmpty()) {
            panel(0).setEnabled(false);                             // receiver has side effects
        } else {
            panel(0).setEnabled(true);
        }
        if (rows.isEmpty()) {
            export.set(next(), false);                              // another argument is a call
        } else {
            export.set(next(), true);
        }
        if (rows.isEmpty()) {
            export.setEnabled(false);
        } else {
            export.setVisible(true);                                // different methods
        }
        if (rows.isEmpty()) {
            export.set(false, true);                                // two literals differ
        } else {
            export.set(true, false);
        }
        if (rows.isEmpty()) {
            export.setEnabled(false);
        } else if (limit > 0) {
            export.setEnabled(true);
        }
        if (boxed) {
            export.setEnabled(true);                                // a Boolean condition
        } else {
            export.setEnabled(false);
        }
        if (rows.isEmpty()) {
            limited |= true;
        } else {
            limited |= false;
        }
        if (rows.isEmpty()) {
            export.setEnabled(true);
        } else {
            export.setEnabled(true);                                // same value
        }
    }

    Panel panel(int i) { return null; }
    int next() { return 0; }
}
