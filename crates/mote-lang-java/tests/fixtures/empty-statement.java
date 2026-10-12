import java.util.Iterator;
import java.util.List;

class Semicolons {
    private int x;; // expect: empty-statement

    void m() {}; // expect: empty-statement

    void positives(boolean ok, List<String> items, int n) {
        ; // expect: empty-statement
        reload();; // expect: empty-statement
        if (ok); // expect: empty-statement
        {
            reload();
        }
        if (ok) reload(); else; // expect: empty-statement
        while (n > 0); // expect: empty-statement
        for (String s : items); // expect: empty-statement
        for (int i = 0; i < n; i++); // expect: empty-statement
        label: ; // expect: empty-statement
        switch (n) {
            case 1:
                reload();; // expect: empty-statement
                break;
            default:
                break;
        }
    }

    Semicolons() {
        super();; // expect: empty-statement
    }

    void negatives(Iterator<String> it, int[] a, int n) {
        for (;;) {
            if (n > 0) break;
        }
        int i;
        for (i = 0; a[i] != 0; i++);          // search loop: the header does the work
        for (;;);
        while (it.next() != null);            // drains in the condition
        do reload(); while (n-- > 0);
        try (AutoCloseable c = open(); AutoCloseable d = open();) {
            reload();
        } catch (Exception e) {
            reload();
        }
        String text = """
            ;;
            """;
    }

    enum E { A, B; }

    enum F {
        A, B;
        void f() {}
    }

    interface I { void f(); }

    AutoCloseable open() { return null; }
    void reload() {}
}; // expect: empty-statement
