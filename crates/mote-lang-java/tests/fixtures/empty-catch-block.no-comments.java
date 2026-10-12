// mote-test: params allow_comment=false
class StrictEmptyCatch {
    void m() {
        try { run(); } catch (Exception e) { /* ignored on purpose */ }   // expect: empty-catch-block
        try { run(); } catch (Exception e) {                               // expect: empty-catch-block
            // still empty
        }
        try { run(); } catch (Exception ignored) { }
        try { run(); } catch (Exception e) { handle(e); }
    }
}
