// mote-test: params file_convention=0
import java.io.File;

class ArchiveEntries {
    // File modes and permissions, where octal is the convention: not reported.
    static final int DEFAULT_FILE_MODE = 0644;
    static final int DEFAULT_DIR_MODE = 040755;
    static final int REGULAR_FILE_MODE = 0100644;
    static final int PERMS_MASK = 07777;
    static final int FILE_TYPE_MODE_MASK = 0170000;
    static final int STICKY_PERMISSIONS = 01777;
    static final int C_ISUID = 0004000;
    private int umask = 022;
    private int mode;

    void configure(Entry entry, File file, Options options) {
        entry.setMode(0644);
        entry.setMode(0123);
        entry.setUnixMode(0100644);
        entry.setMode(REGULAR_FILE_MODE | 0755);
        chmod(file, 0755);
        options.setPermissions(0600);
        mode = 0755;
        this.mode = 040755;
        umask = 0027;
        int perms = entry.getMode() & 07777;
        boolean executable = (entry.getUnixMode() & 0111) != 0;
        boolean sticky = entry.getPermissionBits() == 01777;
        FileMode fm = new FileMode(0644);
        if ((mode & 0170000) == 040000) {
            options.markDirectory();
        }
        assertEquals(0644, entry.getUnixMode() & 07777);
        assertEquals(040755, entry.getUnixPermissionMode());
        assertThat(entry.getMode()).isEqualTo(0100644);
        putOctal(new byte[8], 100, 8, 0644);                        // a classic mode, whatever the call
        assertEquals(0577, entry.getExternalAttributes() >> 16 & 0xFFFF);
    }

    int defaultMode() {
        return 0644;
    }

    // Still reported: decimal-looking values outside a mode context.
    void notModes(Entry entry, Options options) {
        entry.setSize(0100);                                        // expect: octal-literal
        options.setRetries(010);                                    // expect: octal-literal
        int model = 012;                                            // expect: octal-literal
        long offset = 0200;                                         // expect: octal-literal
        assertEquals(0123, entry.getSize());                        // expect: octal-literal
        entry.setMode(012345);                                      // five digits: deliberate
    }

    int timeout() {
        return 030;                                                 // expect: octal-literal
    }

    void chmod(File f, int mode) {
    }

    void putOctal(byte[] b, int offset, int length, int value) {
    }

    void assertEquals(long expected, long actual) {
    }
}
