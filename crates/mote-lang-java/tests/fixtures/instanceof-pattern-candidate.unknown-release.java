// No Java release known (no `java.release`, no build file declaring one): the rule cannot tell
// whether the pattern syntax would compile, so it stays silent.
class Unknown {
    String describe(Object value) {
        if (value instanceof Number) {
            Number n = (Number) value;
            return "number " + n.intValue();
        }
        if (value instanceof CharSequence && ((CharSequence) value).length() > 0) {
            return ((CharSequence) value).toString();
        }
        return "other";
    }
}
