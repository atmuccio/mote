import java.io.IOException;
import java.nio.file.Path;

class Cleanup {
    void deleteTree(Path dir) {
        // The javac trick: the helper (written in Kotlin) throws IOException without declaring
        // it, and a catch of a checked exception the try block cannot throw does not compile.
        try {
            TreeHelper.deleteRecursively(dir);
            if (false) {
                throw new IOException("declares what the helper really throws");
            }
        } catch (IOException e) {
            report(e);
        }
        try {
            TreeHelper.deleteRecursively(dir);
            if (false) throw new IOException("same, without braces");
        } catch (IOException e) {
            report(e);
        }
        try {
            TreeHelper.copy(dir);
            if (false) {
                throw new IOException("first");
            }
        } catch (IOException e) {
            report(e);
        }

        // Switched off until the new audit service is live.
        if (false) {
            audit(dir);
        }
        /* Kept on while the migration runs. */
        if (true) {
            migrate(dir);
        } else {
            skip(dir);
        }
        //noinspection ConstantValue
        if (false) {
            audit(dir);
        }
        if (false) {
            // Old format writer, kept for the rollback plan.
            writeLegacy(dir);
        }
        if (true) {
            migrate(dir);
        } else {
            /* Unreachable until the flag comes back. */
            skip(dir);
        }
    }

    void stillReported(Path dir) throws IOException {
        if (true) {                                                 // expect: constant-if-condition
            throw new IllegalStateException("always thrown");
        }
    }

    void mixedBodies(Path dir) throws IOException {
        if (false) {                                                // expect: constant-if-condition
            audit(dir);
            throw new IOException("mixed body");
        }
        if (false) {                                                // expect: constant-if-condition
            throw new IOException("with an else");
        } else {
            migrate(dir);
        }
        if (false) {                                                // expect: constant-if-condition
        }
    }

    void commentsThatDoNotExplain(Path dir) {
        // A comment separated from the if by a blank line.

        if (false) {                                                // expect: constant-if-condition
            audit(dir);
        }
        if (false) {                                                // expect: constant-if-condition
            audit(dir);                                             // a trailing comment
        }
        migrate(dir);                                               // trailing on the line above
        if (false) {                                                // expect: constant-if-condition
            audit(dir);
        }
    }

    void audit(Path p) {
    }

    void migrate(Path p) {
    }

    void skip(Path p) {
    }

    void writeLegacy(Path p) {
    }

    void report(Exception e) {
    }
}
