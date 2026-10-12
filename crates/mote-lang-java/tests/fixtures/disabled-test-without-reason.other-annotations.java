import com.fasterxml.jackson.annotation.JsonIgnore;
import com.acme.config.Ignore;

// Annotations that merely share the name are not test annotations.
class Dto {
    @Ignore
    private String internal;

    @JsonIgnore
    private String hidden;

    @Disabled                                                       // not imported: unknown origin
    void feature() { }
}
