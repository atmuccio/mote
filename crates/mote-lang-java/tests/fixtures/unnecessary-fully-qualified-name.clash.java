import com.acme.List;

// `List` means com.acme.List here: java.util.List must stay qualified.
class Clash {
    java.util.List<String> names;
    com.acme.List other;                                            // expect: unnecessary-fully-qualified-name

    <String> void generic(java.lang.String s) {                     // `String` is a type variable here
    }
}
