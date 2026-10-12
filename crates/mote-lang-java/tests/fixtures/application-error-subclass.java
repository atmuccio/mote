import java.io.IOError;

class Errors {
    public static class ConfigurationError extends Error {             // expect: application-error-subclass
        public ConfigurationError(String message) {
            super(message);
        }
    }

    static class Fatal extends java.lang.Error {                        // expect: application-error-subclass
    }

    // Compliant
    static class ConfigurationException extends RuntimeException {
    }

    static class CheckFailed extends AssertionError {                   // test-library style failure
    }

    static class Io extends IOError {
        Io(Throwable cause) {
            super(cause);
        }
    }

    static class Plain {
    }
}

class ShadowedError {
    // A class of this file named `Error` (like an API model) is not java.lang.Error.
    static class Error {
        String code;
    }

    static class ValidationError extends Error {
    }
}
