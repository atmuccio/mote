import java.util.List;
import java.util.Map;

class Layout {
    Object convert(Object value, boolean admin, Object user) {
        if (value instanceof Map) {
            use(value);
        } if (value instanceof List) {              // expect: misleading-if-layout
            use(value);
        }
        if (admin) {
            use(value);
        } else {
            use(user);
        } if (user == null) {                        // expect: misleading-if-layout
            use(null);
        }

        if (user != null)
            if (admin)
                use(user);
        else                                         // expect: misleading-if-layout
            use(value);
        return value;
    }

    void negatives(Object value, boolean admin, Object user) {
        if (value == null) {
            use(value);
        } else if (admin) {
            use(user);
        }
        if (value == null) {
            use(value);
        }
        if (admin) {
            use(user);
        }
        if (admin) use(value); if (user == null) use(user);  // braceless: not the `} if` shape
        while (admin) {
            use(value);
        } if (user != null) {                        // previous statement is not an if
            use(user);
        }
        if (value != null) {
            use(value);
        } /* independent */ if (admin) {
            use(user);
        }
        if (user != null)
            if (admin)
                use(user);
            else                                     // aligned with its own if
                use(value);
        if (user != null) {
            if (admin)
                use(user);
        } else
            use(value);
    }

    void use(Object o) {}
}
