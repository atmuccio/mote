public class Mailer {
    private String host;

    // ------------------------------------------------------------ constructors

    public // expect-next-line: redundant-default-constructor
    Mailer() {
    }

    public String host() {
        return host;
    }
}

public class Sampler {
    /* Construction */

    public // expect-next-line: redundant-default-constructor
    Sampler() {
        super();
    }
}

public class Documented {
    // Needed by the bean loader.
    public Documented() {
    }
}

public class DocumentedBlock {
    /** Creates the sampler for the bean loader. */
    public DocumentedBlock() {
    }
}

interface Matchers {
    class Builder {                                                 // expect-next-line: redundant-default-constructor
        public Builder() {
        }

        Builder add(String s) {
            return this;
        }
    }

    class Hidden {
        private Hidden() {
        }
    }
}
