package com.acme.text;

import static com.acme.util.Strings.isBlank;
import static com.acme.util.Strings.join;
import static com.acme.util.Limits.MAX_ROWS;
import static com.acme.util.Limits.MIN_ROWS;
import static com.acme.util.Limits.DEFAULT;
import static java.lang.String.format;
import static java.util.Arrays.toString;
import static java.util.Objects.requireNonNull;
import static com.acme.other.Checks.requireNonNull;
import static com.acme.util.Casts.cast;
import static com.acme.util.Levels.render;

import com.acme.util.Casts;
import com.acme.util.Levels;
import com.acme.util.Limits;
import com.acme.util.Strings;
import java.util.Arrays;
import java.util.Objects;

class Formatter {
    String describe(String name, int rows) {
        if (Strings.isBlank(name)) {                                // expect: unnecessary-fully-qualified-name
            return "";
        }
        if (isBlank(name.trim())) {
            return "?";
        }
        int max = Limits.MAX_ROWS;                                  // expect: unnecessary-fully-qualified-name
        String s = String.format("%s: %d", name, Math.min(rows, max)); // expect: unnecessary-fully-qualified-name
        return Strings.join(",", s, format("%d", MIN_ROWS));        // expect: unnecessary-fully-qualified-name
    }

    String traps(int[] values, Object o) {
        // `toString` is inherited from Object: the bare call would not reach the import.
        String a = Arrays.toString(values);
        // Two static imports of `requireNonNull`: the bare name is ambiguous.
        Objects.requireNonNull(o);
        // Explicit type arguments need the qualifier.
        String c = Casts.<String>cast(o);
        // `DEFAULT` is also a local variable name somewhere in this file.
        Object d = Limits.DEFAULT;
        // `render` is also a method of this file.
        return a + c + d + Levels.render(1);
    }

    String render(int level) {
        String DEFAULT = "x";
        return DEFAULT + level;
    }
}

class Report extends com.acme.base.Page {
    // The superclass may declare its own `isBlank`, which would win over the static import.
    boolean empty(String s) {
        return Strings.isBlank(s);
    }
}

enum Mode {
    MIN_ROWS;

    int rows() {
        // `MIN_ROWS` alone would be this enum's constant.
        return Limits.MIN_ROWS;
    }

    boolean blank(String s) {
        return Strings.isBlank(s);                                  // expect: unnecessary-fully-qualified-name
    }
}

class Listeners {
    Runnable r = new Runnable() {
        public void run() {
            // Inside an anonymous class: its supertype could declare `isBlank`.
            Strings.isBlank("x");
        }
    };

    Runnable lambda = () -> Strings.isBlank("y");                   // expect: unnecessary-fully-qualified-name
}

record Page(String title) {
    boolean blank() {
        return Strings.isBlank(title);
    }
}
